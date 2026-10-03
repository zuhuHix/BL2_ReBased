// Finds the UE3 native-function registration tables in the program and writes a name -> address map.
// Optionally labels every registered native in the database (namespace = C++ class, label = execFunction).
//
// Contains no game data. Everything it writes is game-derived and goes to the output folder passed as the
// first argument, which must be outside the repository or under its ignored local/ (docs/NATIVE_ANALYSIS.md).
//
// Mechanism (found by inspecting strings and references; see docs/NATIVE_ANALYSIS.md "Native registration"):
// each native class has a static table of {const char* "<CppClass>exec<Function>", function pointer} pairs,
// ended by a {null, null} pair. The scan looks, in every initialized non-executable block, for 4-byte aligned
// pairs whose first word points at such a string and whose second word points into executable memory.
//
// Arguments: <outDir> [apply] [inatives:<file>]
//   apply              label every native and rename its function (persisted when run without -readOnly)
//   inatives:<file>    tab-separated "Package.Class.Function<TAB>iNative" lines (tools/ghidra/script_natives.py);
//                      when given, gnatives.tsv maps each script iNative to the native bound by name
//@category OpenWillow
import java.io.File;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;

public class OwNativeTables extends GhidraScript {
    // C++ class (A/U/F/I prefix), the first "exec" followed by an upper-case letter or '_', then the function.
    static final Pattern NAME = Pattern.compile("([AUFI][A-Za-z0-9_]*?)exec([A-Z_][A-Za-z0-9_]*)");

    static final class Entry {
        String raw, cppClass, function;
        long nameAddr, fnAddr, slotAddr;
        int table;
    }

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) throw new IllegalArgumentException("usage: OwNativeTables <outDir> [apply] [inatives:<file>]");
        File out = new File(args[0]);
        out.mkdirs();
        boolean apply = false;
        File inatives = null;
        for (int i = 1; i < args.length; i++) {
            if (args[i].equals("apply")) apply = true;
            else if (args[i].startsWith("inatives:")) inatives = new File(args[i].substring(9));
            else throw new IllegalArgumentException("unknown argument " + args[i]);
        }
        long started = System.currentTimeMillis();
        List<Entry> entries = scan();
        println(String.format("native tables: %d entries in %d tables (%.1f s)", entries.size(),
            entries.isEmpty() ? 0 : entries.get(entries.size() - 1).table + 1, (System.currentTimeMillis() - started) / 1000.0));

        try (PrintWriter w = new PrintWriter(new File(out, "natives.tsv"), StandardCharsets.UTF_8)) {
            w.println("raw\tcppClass\tfunction\taddress\ttable");
            for (Entry e : entries)
                w.printf("%s\t%s\t%s\t%08x\t%d%n", e.raw, e.cppClass, e.function, e.fnAddr, e.table);
        }
        // Consistency: every entry of one table must name the same class.
        int mixed = 0;
        Map<Integer, String> tableClass = new HashMap<>();
        for (Entry e : entries) {
            String c = tableClass.putIfAbsent(e.table, e.cppClass);
            if (c != null && !c.equals(e.cppClass)) mixed++;
        }
        println("tables naming more than one class: " + mixed);

        if (inatives != null) writeGNatives(entries, inatives, new File(out, "gnatives.tsv"));
        if (apply) {
            int labelled = applyNames(entries);
            println("labelled " + labelled + " natives");
        }
    }

    List<Entry> scan() throws Exception {
        Memory memory = currentProgram.getMemory();
        List<Entry> entries = new ArrayList<>();
        Map<Long, String> stringCache = new HashMap<>();
        int table = -1;
        for (MemoryBlock block : memory.getBlocks()) {
            if (!block.isInitialized() || block.isExecute() || block.getSize() > Integer.MAX_VALUE) continue;
            byte[] bytes = new byte[(int) block.getSize()];
            block.getBytes(block.getStart(), bytes);
            long base = block.getStart().getOffset();
            long previousSlot = Long.MIN_VALUE;
            for (int i = 0; i + 8 <= bytes.length; i += 4) {
                long p = u32(bytes, i), fn = u32(bytes, i + 4);
                if (p == 0 || fn == 0) continue;
                MemoryBlock fnBlock = block(fn);
                if (fnBlock == null || !fnBlock.isExecute()) continue;
                MemoryBlock nameBlock = block(p);
                if (nameBlock == null || nameBlock.isExecute() || !nameBlock.isInitialized()) continue;
                String raw = stringCache.computeIfAbsent(p, this::cString);
                if (raw == null) continue;
                Matcher m = NAME.matcher(raw);
                if (!m.matches()) continue;
                long slot = base + i;
                if (slot != previousSlot + 8) table++;
                previousSlot = slot;
                Entry e = new Entry();
                e.raw = raw;
                e.cppClass = m.group(1);
                e.function = m.group(2);
                e.nameAddr = p;
                e.fnAddr = fn;
                e.slotAddr = slot;
                e.table = table;
                entries.add(e);
                i += 4;   // a pair consumes two words
            }
        }
        return entries;
    }

    MemoryBlock block(long offset) {
        try {
            return currentProgram.getMemory().getBlock(toAddr(offset));
        } catch (Exception e) {
            return null;
        }
    }

    String cString(long offset) {
        try {
            Memory memory = currentProgram.getMemory();
            Address a = toAddr(offset);
            StringBuilder s = new StringBuilder();
            for (int n = 0; n < 256; n++) {
                byte b = memory.getByte(a.add(n));
                if (b == 0) return s.length() >= 6 ? s.toString() : null;
                if (b < 0x20 || b > 0x7e) return null;
                s.append((char) b);
            }
            return null;
        } catch (Exception e) {
            return null;
        }
    }

    static long u32(byte[] b, int i) {
        return (b[i] & 0xffL) | (b[i + 1] & 0xffL) << 8 | (b[i + 2] & 0xffL) << 16 | (b[i + 3] & 0xffL) << 24;
    }

    // Script iNative -> native bound by name. A script function Package.Class.Function binds to the table entry
    // whose function name matches and whose C++ class is the script class with its one-letter prefix.
    void writeGNatives(List<Entry> entries, File inatives, File target) throws Exception {
        Map<String, Entry> byKey = new HashMap<>();
        for (Entry e : entries) byKey.put(e.cppClass.substring(1) + "." + e.function, e);
        int bound = 0, unbound = 0;
        try (PrintWriter w = new PrintWriter(target, StandardCharsets.UTF_8)) {
            w.println("iNative\tscriptFunction\traw\taddress");
            for (String line : Files.readAllLines(inatives.toPath(), StandardCharsets.UTF_8)) {
                String[] f = line.split("\t");
                if (f.length < 2 || f[1].equals("0") || !f[1].matches("\\d+")) continue;
                String[] parts = f[0].split("\\.");
                if (parts.length < 3) continue;
                Entry e = byKey.get(parts[parts.length - 2] + "." + parts[parts.length - 1]);
                if (e == null) { unbound++; w.printf("%s\t%s\t\t%n", f[1], f[0]); continue; }
                bound++;
                w.printf("%s\t%s\t%s\t%08x%n", f[1], f[0], e.raw, e.fnAddr);
            }
        }
        println("gnatives: " + bound + " bound by name, " + unbound + " unbound");
    }

    int applyNames(List<Entry> entries) throws Exception {
        SymbolTable symbols = currentProgram.getSymbolTable();
        Namespace global = currentProgram.getGlobalNamespace();
        Map<String, Namespace> namespaces = new HashMap<>();
        int labelled = 0;
        for (Entry e : entries) {
            Namespace ns = namespaces.get(e.cppClass);
            if (ns == null) {
                ns = symbols.getNamespace(e.cppClass, global);
                if (ns == null) ns = symbols.createNameSpace(global, e.cppClass, SourceType.USER_DEFINED);
                namespaces.put(e.cppClass, ns);
            }
            Address a = toAddr(e.fnAddr);
            String label = "exec" + e.function;
            Function f = getFunctionAt(a);
            if (f == null) {
                disassemble(a);
                f = createFunction(a, label);
            }
            boolean named = f != null && f.getSymbol().getSource() == SourceType.USER_DEFINED;
            if (f != null && !named) {
                f.setParentNamespace(ns);
                f.setName(label, SourceType.USER_DEFINED);
            } else if (f == null || !(f.getName().equals(label) && f.getParentNamespace().equals(ns))) {
                // One body can serve several natives (identical code folded by the linker): the first name stays on
                // the function, the others become labels at the same address.
                if (symbols.getSymbol(label, a, ns) == null) symbols.createLabel(a, label, ns, SourceType.USER_DEFINED);
            }
            labelled++;
        }
        return labelled;
    }
}
