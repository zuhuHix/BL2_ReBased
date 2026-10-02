// Resolves natives (or addresses, or string references) and writes their decompilation to an ignored folder.
//
// Contains no game data. Everything it writes is game-derived and goes to the output folder passed as the first
// argument, which must be outside the repository or under its ignored local/ (docs/NATIVE_ANALYSIS.md). It needs
// natives.tsv (and for iNative queries gnatives.tsv) in that folder, written by OwNativeTables.java.
//
// Arguments: <outDir> <query>... ; a query "@file" reads one query per line from that file ('#' starts a comment).
// Query forms:
//   Class.Function          script class and function, e.g. BehaviorKernel.ActivateBehaviorOutputLink
//                           (the C++ class may carry any of the A/U/F/I prefixes)
//   execFunction            every native with that function name
//   UClassexecFunction      the registered name exactly
//   native:<n>              the native bound to script iNative n (needs gnatives.tsv)
//   0x<hex>                 the function containing that address
//   str:<text>              every function referencing a string (ANSI or UTF-16) equal to <text>
//   callers:<query>         the functions calling whatever <query> resolves to (decompiled too)
//   refs:<hex>              the functions whose code references that address (e.g. a global FName)
//   insn:<regex>            the functions containing an instruction whose text matches (first 200);
//                           insn@<lohex>-<hihex>:<regex> limits the search to an address range
//   virtual:<Class>:<hexOffset>[:<Name>]  the function in a script class's C++ vtable at that byte offset (e.g. 148 is
//                           ApplyBehaviorToContext for Behavior subclasses); with label, named <UClass>::<Name>
//   name:<hex>:<Ns::Name>   name the function at that address (persisted); an empty name resets it
// Options (anywhere in the argument list):
//   label                   label resolved natives Class::execFunction (persisted when run without -readOnly)
//   callees:<n>             also decompile direct callees, n levels deep (default 0)
//   timeout:<s>             decompiler timeout per function (default 120)
// Output: <outDir>/decomp/<name>.c per function (with its callers and callees listed at the top) and
// <outDir>/query.tsv (query, resolved name, address, file).
//@category OpenWillow
import java.io.File;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;

public class OwNativeQuery extends GhidraScript {
    static final class Native { String raw, cppClass, function; long address; }

    final List<Native> natives = new ArrayList<>();
    final Map<Integer, Native> byINative = new HashMap<>();
    DecompInterface decompiler;
    File outDir, decompDir;
    boolean label;
    int calleeDepth = 0, timeout = 120;
    final Set<Address> written = new HashSet<>();
    PrintWriter summary;

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) throw new IllegalArgumentException("usage: OwNativeQuery <outDir> <query|@file>... [label] [callees:n]");
        outDir = new File(args[0]);
        decompDir = new File(outDir, "decomp");
        decompDir.mkdirs();
        List<String> queries = new ArrayList<>();
        for (int i = 1; i < args.length; i++) {
            String a = args[i];
            if (a.equals("label")) label = true;
            else if (a.startsWith("callees:")) calleeDepth = Integer.parseInt(a.substring(8));
            else if (a.startsWith("timeout:")) timeout = Integer.parseInt(a.substring(8));
            else if (a.startsWith("@")) {
                for (String line : Files.readAllLines(new File(a.substring(1)).toPath(), StandardCharsets.UTF_8)) {
                    String q = line.replaceAll("#.*", "").trim();
                    if (!q.isEmpty()) queries.add(q);
                }
            } else queries.add(a);
        }
        loadTables();
        decompiler = new DecompInterface();
        decompiler.setOptions(new DecompileOptions());
        decompiler.openProgram(currentProgram);
        long started = System.currentTimeMillis();
        try (PrintWriter w = new PrintWriter(new File(outDir, "query.tsv"), StandardCharsets.UTF_8)) {
            summary = w;
            w.println("query\tname\taddress\tfile");
            for (String q : queries) {
                List<Function> found = resolve(q);
                if (found.isEmpty()) { println("unresolved: " + q); w.printf("%s\t\t\t%n", q); continue; }
                for (Function f : found) emit(q, f, calleeDepth);
            }
        } finally {
            decompiler.dispose();
        }
        println(String.format("%d queries, %d functions written to %s (%.1f s)", queries.size(), written.size(), decompDir,
            (System.currentTimeMillis() - started) / 1000.0));
    }

    void loadTables() throws Exception {
        File tsv = new File(outDir, "natives.tsv");
        if (!tsv.exists()) throw new IllegalStateException("run OwNativeTables.java first: missing " + tsv);
        for (String line : Files.readAllLines(tsv.toPath(), StandardCharsets.UTF_8)) {
            String[] f = line.split("\t");
            if (f.length < 4 || f[0].equals("raw")) continue;
            Native n = new Native();
            n.raw = f[0]; n.cppClass = f[1]; n.function = f[2]; n.address = Long.parseLong(f[3], 16);
            natives.add(n);
        }
        File g = new File(outDir, "gnatives.tsv");
        if (g.exists()) {
            Map<String, Native> byRaw = new HashMap<>();
            for (Native n : natives) byRaw.put(n.raw, n);
            for (String line : Files.readAllLines(g.toPath(), StandardCharsets.UTF_8)) {
                String[] f = line.split("\t");
                if (f.length < 4 || !f[0].matches("\\d+") || f[2].isEmpty()) continue;
                Native n = byRaw.get(f[2]);
                if (n != null) byINative.put(Integer.parseInt(f[0]), n);
            }
        }
    }

    List<Function> resolve(String q) throws Exception {
        List<Function> result = new ArrayList<>();
        if (q.startsWith("callers:")) {
            Set<Function> callers = new LinkedHashSet<>();
            for (Function target : resolve(q.substring(8)))
                callers.addAll(target.getCallingFunctions(monitor));
            result.addAll(callers);
            return result;
        }
        if (q.startsWith("str:")) return stringUsers(q.substring(4));
        if (q.startsWith("insn:") || q.startsWith("insn@")) {
            // Functions containing an instruction whose listing text matches a regex, e.g. insn:OR dword ptr \[\w+ \+ 0x\w+\],0x4;
            // insn@<lohex>-<hihex>:<regex> searches only that address range.
            String body = q.substring(5);
            Address lo = currentProgram.getMinAddress(), hi = currentProgram.getMaxAddress();
            if (q.startsWith("insn@")) {
                int colon = body.indexOf(':');
                String[] range = body.substring(0, colon).split("-");
                lo = toAddr(Long.parseLong(range[0].replace("0x", ""), 16));
                hi = toAddr(Long.parseLong(range[1].replace("0x", ""), 16));
                body = body.substring(colon + 1);
            }
            java.util.regex.Pattern pattern = java.util.regex.Pattern.compile(body);
            Set<Function> users = new LinkedHashSet<>();
            for (Instruction ins : currentProgram.getListing().getInstructions(
                     new ghidra.program.model.address.AddressSet(lo, hi), true)) {
                if (!pattern.matcher(ins.toString()).find()) continue;
                Function f = getFunctionContaining(ins.getAddress());
                if (f != null) users.add(f);
                if (users.size() >= 200) break;
            }
            result.addAll(users);
            return result;
        }
        if (q.startsWith("refs:")) {
            // Functions whose code references an address (e.g. a global holding a script event's FName).
            Set<Function> users = new LinkedHashSet<>();
            for (Reference r : getReferencesTo(toAddr(Long.parseLong(q.substring(5).replace("0x", ""), 16)))) {
                Function f = getFunctionContaining(r.getFromAddress());
                if (f != null) users.add(f);
            }
            result.addAll(users);
            return result;
        }
        if (q.startsWith("name:")) {
            // name:<hex>:<Namespace::Name> names the function at that address; an empty name resets it to the default.
            String[] p = q.split(":", 3);
            Function f = functionAt(Long.parseLong(p[1].replace("0x", ""), 16));
            if (f == null) return result;
            String full = p.length > 2 ? p[2] : "";
            if (full.isEmpty()) {
                f.setParentNamespace(currentProgram.getGlobalNamespace());
                f.setName(null, SourceType.DEFAULT);
            } else {
                Namespace ns = currentProgram.getGlobalNamespace();
                String simple = full;
                int split = full.lastIndexOf("::");
                if (split > 0) {
                    SymbolTable symbols = currentProgram.getSymbolTable();
                    String nsName = full.substring(0, split);
                    Namespace found = symbols.getNamespace(nsName, ns);
                    ns = found != null ? found : symbols.createNameSpace(ns, nsName, SourceType.USER_DEFINED);
                    simple = full.substring(split + 2);
                }
                f.setParentNamespace(ns);
                f.setName(simple, SourceType.USER_DEFINED);
            }
            result.add(f);
            return result;
        }
        if (q.startsWith("virtual:")) {
            String[] p = q.split(":");
            Function f = virtualFunction(p[1], Long.parseLong(p[2].replace("0x", ""), 16), p.length > 3 ? p[3] : null);
            if (f != null) result.add(f);
            return result;
        }
        if (q.startsWith("0x")) {
            Function f = functionAt(Long.parseLong(q.substring(2), 16));
            if (f != null) result.add(f);
            return result;
        }
        List<Native> matches = new ArrayList<>();
        if (q.startsWith("native:") || q.startsWith("native_")) {
            Native n = byINative.get(Integer.parseInt(q.substring(7)));
            if (n != null) matches.add(n);
        } else if (q.contains(".")) {
            String cls = q.substring(0, q.lastIndexOf('.')), fn = q.substring(q.lastIndexOf('.') + 1);
            if (cls.contains(".")) cls = cls.substring(cls.lastIndexOf('.') + 1);
            for (Native n : natives)
                if (n.function.equals(fn) && n.cppClass.substring(1).equals(cls)) matches.add(n);
        } else if (q.startsWith("exec")) {
            for (Native n : natives) if (("exec" + n.function).equals(q)) matches.add(n);
        } else {
            for (Native n : natives) if (n.raw.equals(q)) matches.add(n);
        }
        for (Native n : matches) {
            Function f = functionAt(n.address);
            if (f == null) continue;
            if (label) applyLabel(n, f);
            result.add(f);
        }
        return result;
    }

    Function functionAt(long offset) throws Exception {
        Address a = toAddr(offset);
        Function f = getFunctionContaining(a);
        if (f == null) {
            disassemble(a);
            f = createFunction(a, null);
        }
        return f;
    }

    void applyLabel(Native n, Function f) throws Exception {
        SymbolTable symbols = currentProgram.getSymbolTable();
        Namespace global = currentProgram.getGlobalNamespace();
        Namespace ns = symbols.getNamespace(n.cppClass, global);
        if (ns == null) ns = symbols.createNameSpace(global, n.cppClass, SourceType.USER_DEFINED);
        String name = "exec" + n.function;
        if (f.getSymbol().getSource() != SourceType.USER_DEFINED) {
            f.setParentNamespace(ns);
            f.setName(name, SourceType.USER_DEFINED);
        } else if (symbols.getSymbol(name, f.getEntryPoint(), ns) == null) {
            symbols.createLabel(f.getEntryPoint(), name, ns, SourceType.USER_DEFINED);
        }
    }

    // The function in a script class's vtable at a byte offset (a virtual native such as ApplyBehaviorToContext:
    // its exec thunk is shared by every class and calls through that slot). UE3 registers each class with its
    // name as a UTF-16 string and an in-place constructor; the constructor's last store of a data address
    // through its `this` register is the class's vtable. Returns null (and prints why) when the pattern fails.
    Function virtualFunction(String scriptClass, long offset, String name) throws Exception {
        Listing listing = currentProgram.getListing();
        Set<Long> nameAddresses = new HashSet<>();
        for (Address a : stringAddresses(scriptClass, true)) nameAddresses.add(a.getOffset());
        for (Function registrar : stringUsers(scriptClass, true)) {
            // Arguments are pushed right to left, so the constructor pointers are pushed after the previous call
            // and before the class name.
            List<Function> pushed = new ArrayList<>();
            List<Function> candidates = new ArrayList<>();
            for (Instruction ins : listing.getInstructions(registrar.getBody(), true)) {
                if (ins.getMnemonicString().equals("CALL")) { pushed.clear(); continue; }
                if (!ins.getMnemonicString().equals("PUSH") || ins.getScalar(0) == null) continue;
                long value = ins.getScalar(0).getUnsignedValue();
                if (nameAddresses.contains(value)) { candidates.addAll(pushed); break; }
                Function pushedFunction = getFunctionAt(toAddr(value));
                if (pushedFunction != null) pushed.add(pushedFunction);
            }
            for (Function ctor : candidates) {
                if (ctor.getBody().getNumAddresses() > 64) continue;
                Long vtable = null;
                for (Instruction c : listing.getInstructions(ctor.getBody(), true)) {
                    if (!c.getMnemonicString().equals("MOV") || c.getNumOperands() != 2 || c.getScalar(1) == null) continue;
                    Object[] dst = c.getOpObjects(0);
                    if (dst.length != 1 || !(dst[0] instanceof ghidra.program.model.lang.Register)) continue;
                    if ((c.getOperandType(0) & ghidra.program.model.lang.OperandType.DYNAMIC) == 0) continue;
                    long value = c.getScalar(1).getUnsignedValue();
                    MemoryBlock b = currentProgram.getMemory().getBlock(toAddr(value));
                    if (b != null && b.isInitialized() && !b.isExecute()) vtable = value;
                }
                if (vtable == null) continue;
                // A vtable is a run of code pointers: check its first slots and the requested one. Script-only classes
                // (no C++ class) fail here instead of yielding a wrong function.
                boolean valid = true;
                for (long o = 0; o <= 12 && valid; o += 4) valid = isCodePointer(vtable + o);
                if (!valid || !isCodePointer(vtable + offset)) {
                    println(String.format("virtual: %s+0x%x does not look like a vtable slot (script-only class?)", scriptClass, offset));
                    continue;
                }
                long slot = currentProgram.getMemory().getInt(toAddr(vtable + offset)) & 0xffffffffL;
                Function inside = getFunctionContaining(toAddr(slot));
                if (inside != null && !inside.getEntryPoint().equals(toAddr(slot))) {
                    println(String.format("virtual: %s+0x%x points into the middle of %s (script-only class?)", scriptClass,
                        offset, inside.getName(true)));
                    continue;
                }
                Function f = getFunctionAt(toAddr(slot));
                if (f == null) { disassemble(toAddr(slot)); f = createFunction(toAddr(slot), null); }
                if (f != null && label && name != null && f.getSymbol().getSource() != SourceType.USER_DEFINED) {
                    SymbolTable symbols = currentProgram.getSymbolTable();
                    Namespace global = currentProgram.getGlobalNamespace();
                    String cpp = "U" + scriptClass;
                    Namespace ns = symbols.getNamespace(cpp, global);
                    if (ns == null) ns = symbols.createNameSpace(global, cpp, SourceType.USER_DEFINED);
                    f.setParentNamespace(ns);
                    f.setName(name, SourceType.USER_DEFINED);
                }
                println(String.format("virtual %s+0x%x: vtable %08x -> %08x", scriptClass, offset, vtable, slot));
                return f;
            }
        }
        println("virtual: no registration/constructor pattern for " + scriptClass);
        return null;
    }

    // The word at `at` points exactly at a disassembled instruction or a known function entry. Real vtable slots do;
    // a word that merely lands in code (a script-only class's "vtable" read from unrelated data) usually does not.
    boolean isCodePointer(long at) {
        try {
            Address target = toAddr(currentProgram.getMemory().getInt(toAddr(at)) & 0xffffffffL);
            MemoryBlock b = currentProgram.getMemory().getBlock(target);
            return b != null && b.isExecute()
                && (getFunctionAt(target) != null || currentProgram.getListing().getInstructionAt(target) != null);
        } catch (Exception e) {
            return false;
        }
    }

    List<Function> stringUsers(String text) throws Exception { return stringUsers(text, false); }

    // Functions whose code references a string equal to text (ANSI or UTF-16; UTF-16 only when wideOnly).
    List<Function> stringUsers(String text, boolean wideOnly) throws Exception {
        Set<Function> users = new LinkedHashSet<>();
        for (Address hit : stringAddresses(text, wideOnly))
            for (Reference r : getReferencesTo(hit)) {
                Function f = getFunctionContaining(r.getFromAddress());
                if (f != null) users.add(f);
            }
        return new ArrayList<>(users);
    }

    // Every place in initialized data where text is stored NUL-terminated.
    List<Address> stringAddresses(String text, boolean wideOnly) throws Exception {
        List<Address> hits = new ArrayList<>();
        Memory memory = currentProgram.getMemory();
        List<byte[]> needles = wideOnly ? List.of((text + "\0").getBytes(StandardCharsets.UTF_16LE))
                                        : List.of((text + "\0").getBytes(StandardCharsets.US_ASCII),
                                                  (text + "\0").getBytes(StandardCharsets.UTF_16LE));
        for (MemoryBlock block : memory.getBlocks()) {
            if (!block.isInitialized() || block.isExecute()) continue;
            for (byte[] needle : needles) {
                Address from = block.getStart();
                while (from != null && from.compareTo(block.getEnd()) <= 0) {
                    Address hit = memory.findBytes(from, block.getEnd(), needle, null, true, monitor);
                    if (hit == null) break;
                    hits.add(hit);
                    from = hit.add(1);
                }
            }
        }
        return hits;
    }

    void emit(String query, Function f, int depth) throws Exception {
        String name = f.getName(true).replace("::", "_").replaceAll("[^A-Za-z0-9_.-]", "_");
        String file = name + "_" + f.getEntryPoint().toString() + ".c";
        // Several natives can share one body (identical code folded by the linker): every query is listed.
        summary.printf("%s\t%s\t%s\t%s%n", query, f.getName(true), f.getEntryPoint(), file);
        if (!written.add(f.getEntryPoint())) return;
        Set<Function> callees = f.getCalledFunctions(monitor);
        Set<Function> callers = f.getCallingFunctions(monitor);
        DecompileResults r = decompiler.decompileFunction(f, timeout, monitor);
        try (PrintWriter w = new PrintWriter(new File(decompDir, file), StandardCharsets.UTF_8)) {
            w.println("// GAME-DERIVED decompiler output. Never commit, never paste (docs/LEGAL.md).");
            w.println("// query: " + query + "  function: " + f.getName(true) + " @ " + f.getEntryPoint());
            for (Function c : callers) w.println("// caller: " + c.getName(true) + " @ " + c.getEntryPoint());
            for (Function c : callees) w.println("// callee: " + c.getName(true) + " @ " + c.getEntryPoint());
            if (r != null && r.decompileCompleted()) w.println(r.getDecompiledFunction().getC());
            else w.println("// decompilation failed: " + (r == null ? "no result" : r.getErrorMessage()));
        }
        if (depth > 0)
            for (Function c : callees)
                if (!c.isThunk() && !c.isExternal()) emit(query + " > callee", c, depth - 1);
    }
}
