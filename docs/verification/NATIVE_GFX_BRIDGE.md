# Native GFx bridge: UnrealScript to Scaleform movies and back (2026-10-05)

AI-assisted (Claude), analyst lane G5. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Nothing in this note was confirmed in the running game.

Scope: the natives of `GFxMoviePlayer`, `GFxObject`, `GearboxGFxMovie`, `WillowGFxMovie3D` and
`QuestAcceptGFxMovie` that the mission UI script calls (`QuestAcceptGFxMovie`, the HUD mission widget, the reward
screen; ranking in [NATIVE_SLICE_CENSUS.md](NATIVE_SLICE_CENSUS.md)). The host runs the real movies under Ruffle with a
JS adapter (`tools/hud_overlay/`, `tools/hud_harness_swf.py`,
[INVENTORY_MOVIE_PROTOTYPE.md](INVENTORY_MOVIE_PROTOTYPE.md)); this note describes the contract the adapter has to honour.
Mission and controller natives belong to lanes C1/G4 ([NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md),
[NATIVE_MISSION_DISPATCH.md](NATIVE_MISSION_DISPATCH.md)), the Engine core natives to G3
([NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md)).

Method and caveats. The registered natives were read directly. Scaleform itself (the GFx movie player, its `Value`
type, `Invoke`, `SetVariable`, the AS2 virtual machine) is compiled into the executable and was **not** read; where a
rule depends on Scaleform's own behaviour (path resolution, `ToString`/`ToBoolean` conversion, sticky variables) it is
described from public Scaleform behaviour and marked "Scaleform, not read". The "ActionScript*" natives and the
function-handler that reaches script both depend on the engine's script frame layout, which is described only by its
effect. Offsets of `QuestAcceptGFxMovie` were named with the script field list (`MissionTextList`, `MissionCategories`,
`MissionList`); `tools/ghidra/class_layout.py` cannot lay out `GFxMoviePlayer` and its subclasses (a map property in a
parent stops it), so those field names rest on script usage, not on the layout oracle.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| GFxMoviePlayer.ActionScript / ActionScriptVoid | `native final function ActionScript(string Path)` / `ActionScriptVoid(string Path)` | Fire: `SetQuestTitle`, `SetPlayerXP`, `SetLevelAndDifficulty`, every `Set*` of the HUD and reward screens | high (arguments), medium (Scaleform side) | UNVERIFIED |
| GFxMoviePlayer.ActionScriptInt / Float / String / Object | `... int/float/string/GFxObject ActionScriptX(string Path)` | getters in HUD and menus | high | UNVERIFIED |
| GFxObject.ActionScriptVoid / Int / Float / String / Object / Array | `native final function ActionScriptVoid(string Method)` etc. | every `GFxObject` subclass in the HUD and reward cards | high | UNVERIFIED |
| GFxMoviePlayer.ActionScriptSetFunction, GFxObject.ActionScriptSetFunction / ActionScriptSetFunctionOn | `(string Member)` / `(GFxObject Target, string Member)` | binding script delegates into movies | medium | UNVERIFIED |
| GFxObject.SetFunction | `native final function SetFunction(string Member, Object Context, name fname)` | 95 call sites bind `ext*` callbacks | high | UNVERIFIED |
| (engine side) AS to script calls: function handlers and the ExternalInterface callback | n/a | how every `ext*` function in `QuestAcceptGFxMovie` is reached | medium-high | UNVERIFIED |
| GFxMoviePlayer.Invoke | `native final function ASValue Invoke(string Method, array<ASValue> args)` | callers needing a result | high | UNVERIFIED |
| GFxObject.Invoke | `native final function ASValue Invoke(string Member, array<ASValue> args)` | rare | medium | UNVERIFIED |
| GFxMoviePlayer.SetVariable / SetVariableBool / Number / String / Object | `(string Path, <value>)` | Fire: `InitForPC` sets `missions.pcCloseButton._visible` | high | UNVERIFIED |
| GFxMoviePlayer.GetVariable / GetVariableBool / Number / String | `(string Path)` returning the value | menus | high | UNVERIFIED |
| GFxMoviePlayer.GetVariableObject | `native final function GFxObject GetVariableObject(string Path, optional class<GFxObject> Type)` | Fire: `SetFocus` (`missions.selections.selection<N>`) | high | UNVERIFIED |
| GFxMoviePlayer.SetVariableArray / GetVariableArray and the Int/Float/String array variants | `(string Path, int Index, array<...> Arg)` | not read | low | UNVERIFIED |
| GFxMoviePlayer.CreateObject / CreateArray | `GFxObject CreateObject(string ASClass, optional class<GFxObject> Type)` / `GFxObject CreateArray()` | list data | high | UNVERIFIED |
| GFxMoviePlayer.Start / Advance / SetPause / Close | `bool Start(optional bool StartPaused)`, `Advance(float Time)`, `SetPause(optional bool bPausePlayback)`, `Close(optional bool Unload)` | Fire: open and close of the accept screen | medium-high | UNVERIFIED |
| GFxMoviePlayer.GetPC / GetLP | `PlayerController GetPC()` / `LocalPlayer GetLP()` | Fire: `Start`, `extAcceptConfirmed` | high | UNVERIFIED |
| GFxMoviePlayer.RegisterGFxObject / UnregisterGFxObject | `(GFxObject anObject)` | object lifetime | medium | UNVERIFIED |
| GFxMoviePlayer.ResolveDataStoreMarkup | `native final function string ResolveDataStoreMarkup(string Markup)` | not seen in the mission screens | low | UNVERIFIED |
| GFxMoviePlayer.SetWidgetPathBinding | `(GFxObject WidgetToBind, name Path)` | CLIK widget init | low | UNVERIFIED |
| GFxObject.Get / GetBool / GetFloat / GetString / GetObject / GetText | `(string Member)` ... | reward cards | medium-high | UNVERIFIED |
| GFxObject.Set / SetBool / SetFloat / SetString / SetObject / SetText | `(string Member, <value>)`; `SetString` and `SetText` take an optional `TranslationContext` | reward cards, list text | high | UNVERIFIED |
| GFxObject.TranslateString | `string TranslateString(string StringToTranslate, optional TranslationContext InContext)` | markup in text | medium-high | UNVERIFIED |
| GFxObject.GetElement* / SetElement* / CreateEmptyMovieClip / AttachMovie / GotoAndPlay/Stop / SetVisible / SetPosition | see "Not read yet" | list widgets | low | UNVERIFIED |
| GearboxGFxMovie.PlayUISound | `native final function PlayUISound(name UIEvent)` | Fire: `MenuOpen`, `accept_mission`, `MenuClose` | medium-high | UNVERIFIED |
| GearboxGFxMovie.SingleArgInvokeS / F / B | `(string Command, string/float/bool Arg)` | 54 / 8 / 2 call sites (HUD) | medium | UNVERIFIED |
| GearboxGFxMovie.GetInstanceContextObject / GetLocalPlayer / InitFromDefinition / GFxColoredText | see the sections | Fire: `Start` | medium-high | UNVERIFIED |
| WillowGFxMovie3D.FocusOn | `native function FocusOn(GFxObject Thing)` | Fire: `SetFocus` in the accept screen | low-medium | UNVERIFIED |
| QuestAcceptGFxMovie.UpdateMissionTextList | `native function UpdateMissionTextList()` | Fire: list category headers | medium | UNVERIFIED |

Counts of call sites are occurrences in `local/disasm/willowgame_all.txt` (WillowGame script only).

## Conventions: the value type, wrapper objects, gating

**ActionScript value types.** The engine passes values to and from Scaleform in one tagged value type with these kinds:
undefined, null, boolean, number (always a 64-bit float), string (UTF-8) or wide string (UTF-16), object, array, display
object (a movie clip or text field). The script struct `GFxMoviePlayer.ASValue` carries a `Type` of `ASType` (0 undefined,
1 null, 2 number, 3 string, 4 boolean, 5 max; names `AS_Undefined ... AS_Boolean`, confirmed from the package) and the
fields `B` (bool), `N` (float), `S` (string). Scaleform's own tags are different numbers; do not mix them.

**Paths and names are text.** Every path, member and method name is a script `string` (UTF-16) that is converted to UTF-8
for Scaleform. Paths are plain dotted paths such as `missions.pcCloseButton._visible`; a movie that is not wrapped by
the harness resolves them from the root timeline (Scaleform, not read). Nothing in the engine adds or strips a prefix.

**Gate on a missing movie.** All movie-level natives start with the same test: the player must have a loaded movie
(`GFxMoviePlayer` keeps it in its first own field) and the GFx manager (a global) must exist. If not, the native does
nothing: setters are dropped, getters return the type's zero, object getters return `None`, the call is not logged and
the script call itself is not an error. `GFxObject` natives test that the object holds a live object/array/display-object
value; otherwise they do nothing, and getters return the zero. After `Close` every wrapper therefore becomes inert.

**Wrapper objects (`GFxObject`).** A `GFxObject` is a plain script object whose first native field is one ActionScript
value (an object, array or clip), its `Outer` is the movie player that created it. Every native that returns an object
(`GetVariableObject`, `GFxObject.GetObject`, `CreateObject`, `CreateArray`, `ActionScriptObject`, `GetElementObject` and
the like) **allocates a new wrapper each time**: two calls for the same path return two different script objects that
refer to the same ActionScript object (compare by identity of the ActionScript value, not by the wrapper). The class of the
wrapper is the optional `Type` argument, else `GFxObject`. The return is `None` when the path does not exist or the value
is undefined; a path that exists but holds `null` still yields a wrapper (Scaleform, not read). Each wrapper is also
inserted into a set kept by the movie player (de-duplicated by pointer), so it lives as long as the player's session; see
`Close`.

**Locale of numbers.** Script numbers are converted to ActionScript numbers as `double(value)`; from ActionScript back to
an `int` the double is truncated toward zero (a C style cast), to a `byte` the low 8 bits of the truncated value, to a
`float` it is narrowed. There is no rounding.

## The ActionScript* family (the central contract)

These natives have a path or member name as their only declared parameter, but they send **the parameters of the script
function that called them**. This is the idiom of every `Set*` helper:

```
function SetPlayerXP(string Text, int Current, int Max) { ActionScript("SetPlayerXP"); }   // sketch, own words
```

* **Signature:** `GFxMoviePlayer.ActionScript` and `ActionScriptVoid` (the same native), `ActionScriptInt`,
  `ActionScriptFloat` (the same native), `ActionScriptString`, `ActionScriptObject`, `ActionScriptSetFunction`;
  `GFxObject.ActionScriptVoid`, `ActionScriptInt`/`Float` (same native), `ActionScriptString`, `ActionScriptArray` and
  `ActionScriptObject` (the same native), `ActionScriptSetFunction`, `ActionScriptSetFunctionOn`.
* **Reads:** the *calling* script function: its declared parameters in declaration order, their types and current values
  (in the C++ VM these are the caller frame's locals), and its return property.
* **Does (in order):** reads the path string (movie level) or method name (object level); skips the call's end token;
  verifies a script function is executing and that a movie (or a live object value) exists; builds one ActionScript
  argument for **each parameter of the calling function except the return value**, in order (out parameters and optional
  parameters included, with their current values); invokes the method; and, if the native is one of the value-returning
  variants and the caller has a return property, converts the result into that property; finally releases the arguments.
  Movie level: the method name is given to Scaleform's invoke on the movie (a bare name is resolved against the root
  timeline: Scaleform, not read), so the target of `this` is whatever Scaleform decides. Object level: the member of **this
  object's** ActionScript value is invoked with that object as `this`.
* **Result type comes from the caller.** The value is converted to the type of the calling function's return property,
  not to the native's own return type. `ActionScriptInt`, `Float`, `String`, `Object`, `Array` therefore differ only in the
  declared type the script compiler checks. `ActionScript`/`ActionScriptVoid` never produce a result.
* **Argument conversion (script to ActionScript), by property kind:**
  * byte and int become number, float becomes number (widened to double);
  * bool becomes boolean (the bit mask of a packed bool is honoured);
  * string becomes a wide string (UTF-16, not escaped and not translated);
  * object: `None` and any object that is not a `GFxObject` become **null**; a `GFxObject` passes the very ActionScript value
    it wraps (so the movie sees the same object, not a copy);
  * struct (when a movie exists): a new plain ActionScript object is created and **one member is set per struct field**,
    named exactly like the script field, with the field converted by these same rules (recursively). `Color`, `Vector` and
    other structs follow this rule, so `Color` is `{B, G, R, A}` members in field order of the package;
  * dynamic array: a new ActionScript array, elements appended in order, each converted by these rules; a fixed-size array
    property (`ArrayDim` greater than 1) is passed as an array too;
  * any other kind (name, delegate, interface, ...) becomes undefined (no conversion branch was found).
  A second boolean branch exists for property classes carrying another cast flag; which class that is was not identified.
* **Result conversion (ActionScript to the caller's return property):**
  * number to byte (truncate, low 8 bits), int (truncate), float (narrow); to any other property kind the result is dropped
    and the return stays at its zero;
  * boolean to bool; to string (a non-string value yields the empty string, **not** "true"/"false");
  * string (either kind; UTF-8 decoded) to string property; number to string property is **ignored** (the return stays "");
  * array to a dynamic array: the script array is cleared, resized to the ActionScript length and each element is
    converted with the element's property (so arrays of numbers, strings and objects work); to a fixed array: the first
    `min(length, ArrayDim)` elements;
  * object to an object property whose class derives from `GFxObject`: a new wrapper of that class (see above);
    undefined gives `None`;
  * object to a struct property: the members of the ActionScript object are matched to struct fields by name and filled in
    (low confidence: the routine is deferred through a helper that was not read).
* **Missing path, missing member, closed movie:** the invoke returns failure or undefined; the arguments are still built
  and released; the return stays at zero. No script error, no log. An argument conversion never fails.
* **Synchronous.** The call returns after Scaleform's invoke returns; ActionScript runs inside it. If that ActionScript
  calls back into script (see below) the script function runs nested, before the native returns.
* **Object lifetime.** The argument values are created for the call and released at its end (an AS object created for a
  struct argument is not kept by the engine). Wrappers returned as results belong to the movie player.
* **Edge cases:** a native that is called from a function with no parameters sends no arguments; `ActionScriptObject` on
  a function that returns a `GFxObject` of a subclass wraps with that subclass; the movie-level version needs the movie
  loaded, the object-level version needs the object to be an object, array or display object.
* **Implementer checklist:**
  1. Provide "the calling frame" to these natives: the enclosing function's parameter list (types, values) and its return
     property. If the VM cannot give them, these natives cannot be implemented as ordinary natives.
  2. Convert arguments with the table above and call the host adapter as `apply(target, method, args)`; for movie level the
     target is the root, for object level the object's path/handle.
  3. Convert the adapter's return with the result table, into the caller's return type, and leave zero on failure.
  4. Create a new wrapper per returned object, keep a set on the player, release it on `Close`.
  5. Never raise a script error for a missing path.
* **Open:** how Scaleform resolves a bare name for movie level; whether argument counts above the movie's accepted count
  are truncated (the engine passes exactly the parameter count).

### ActionScriptSetFunction, ActionScriptSetFunctionOn, SetFunction

* **SetFunction(Member, Context, fname):** if this wrapper holds an object/array/display object, `Context` is not `None`
  and `fname` is a real name, creates an ActionScript function value whose body calls the script function `fname` on
  `Context`, and stores it as member `Member` of this object. Otherwise it does nothing. Replacing a member replaces the
  binding.
* **ActionScriptSetFunction(Member):** the calling function's **first parameter must be a delegate** (an object plus a
  function name). The native binds that delegate as member `Member` of this object; nothing happens when the delegate is
  unset or the object is not an object/array/clip. `GFxMoviePlayer.ActionScriptSetFunction` does the same on the movie
  player's own target object given as first parameter; `ActionScriptSetFunctionOn(Target, Member)` binds on `Target`.
  (Medium confidence: the delegate layout was read from the storage offsets, not from a declaration.)
* **When the bound member is called from ActionScript** (the same machinery serves `ExternalInterface`, see the next
  section): the handler runs this sequence. If the context object has been destroyed (pending kill) the binding clears
  itself and nothing happens. Otherwise the target object is the context (or the movie player if the binding had no
  context); the function is looked up by name on the target's class (not found: silently nothing); a zeroed parameter
  buffer of the function's size is made; the ActionScript arguments are converted **into the function's parameters in
  order using the result-conversion table above** (arguments beyond the function's parameter count are ignored, missing
  ones stay zero or empty); the function is run as an ordinary script call (so state, `Super` and events apply); then the
  function's return value is converted to ActionScript with the argument table (and handed to Scaleform as the call's
  return); finally every parameter is destroyed.

## How `ext*` callbacks reach script (ActionScript to UnrealScript)

Three routes were found, all by name:

1. **ExternalInterface.** The movie calls `ExternalInterface.call("extFoo", args...)`. The engine's callback receives the
   method name (UTF-8) and the argument values. If the name starts with an underscore it is treated as an internal
   data-provider message (`__registerModel`, `__registerControl`, `__requestItemAt`, `__requestItemRange`, `__handleEvent`:
   the CLIK list/data protocol; not read further). Otherwise the target is the script property
   `GFxMoviePlayer.ExternalInterface` (an object: either the movie player itself or an instance of the
   `ExternalInterfaceClass` named by the `GFxMovieDefinition`; `WillowPlayerController.ClientGFxPlayMovie` sets it, and
   `SetExternalInterface` is plain script). The function named exactly like the method is looked up on that object's class
   (the name is converted to an engine name, so lookup is **case-insensitive**, as engine names are), run with the
   converted arguments, and its return value becomes the call's return. An unknown name, an empty target or a destroyed target is silently ignored.
   This is how `QuestAcceptGFxMovie.extAcceptConfirmed`, `extCompleteConfirmed`, `extPopulateQuestEntries`,
   `extGenericButtonClicked` (`ActionName` as a name parameter), `extNavigateUp/Down`, `extOnClickedChoice`,
   `extChoiceConfirmed`, ... are invoked: the movie names them, no registration happens in script.
   `GearboxGFxMovie.extIsMouseablePlatform` returns a bool to the movie; `extSetLanguageExt(ClipPath)` is also called from
   the movie and answers by invoking `<ClipPath>.SetLanguageExt` with the game language through `SingleArgInvokeS`.
2. **Explicit bindings** with `GFxObject.SetFunction` (95 call sites, e.g. `extOnSetActive`, `requestItemAt`): the movie
   calls a member of an object; the call reaches the context object's script function as described above. The `ext`
   prefix is a convention only.
3. **`fscommand`.** An ActionScript `fscommand("cmd", "arg")` raises the script event `FSCommand(string, string)` on the
   movie player and uses its return as the call's return. Two further hooks the engine installs on every started movie:
   `_global.CLIK_loadCallback` and `_global.CLIK_unloadCallback`, which raise `WidgetInitialized` and `WidgetUnloaded`
   (`name WidgetName, name WidgetPath, GFxObject Widget`, returning bool) and remember that `PostWidgetInit` is due; and
   `_global.gfxProcessSound`, which plays a UI sound by event name.
* **Implementer checklist:** the Ruffle adapter already has the equivalent of route 2 (`forward`) and a general `ow(...)`
  callback. For route 1 the adapter must register `ExternalInterface` callbacks for every `ext*` name the movie
  uses (or one catch-all that receives the name), and call the VM function of that name on the player's
  `ExternalInterface` object with the argument-to-parameter conversion above, returning the converted result. Unknown names
  must return undefined. Calls arrive synchronously from inside ActionScript, possibly while the VM is itself inside a
  `ActionScript*` call (nested), so the VM must be re-entrant at this boundary.
* **Open:** whether the callback also strips or maps a prefix; whether names are matched with a different case rule than
  engine names; the string-argument path for `fscommand` beyond the two arguments.

## Invoke, SetVariable, GetVariable (movie level)

* **GFxMoviePlayer.Invoke(Method, args):** converts each `ASValue` in `args` by its `Type`: null to null, number to number
  (`N`), string to wide string (`S`), boolean to boolean (`B`), undefined and anything else to undefined. Calls
  Scaleform's invoke with the method path and returns the result as an `ASValue`: null, boolean, number or string
  (UTF-8 or wide) map to types 1, 4, 2, 3; an **object or array result becomes undefined** (use `GetVariableObject` or the
  `ActionScript*` family to obtain objects). A failed invoke, a missing movie or an empty gate returns an all-zero
  `ASValue` (undefined). Synchronous.
* **SetVariable(Path, Arg):** converts `Arg` by `Type` (`AS_Null` null; `AS_Number` the float `N` widened; `AS_String`
  the string `S` as a wide string; `AS_Boolean` the bool `B`; anything else undefined) and sets the variable. Missing
  movie: nothing. The set uses mode 1 (sticky in public Scaleform: a value set on a path that does not exist yet is held
  and applied when the clip appears; Scaleform, not read). A set on an existing path is immediate.
* **SetVariableBool / SetVariableNumber / SetVariableString / SetVariableObject:** the same, with boolean, number
  (`float` widened), wide string and the wrapped value of a `GFxObject` (`None` sets undefined).
* **GetVariable(Path):** asks Scaleform for the variable, returns an `ASValue`: null becomes type 1, boolean type 4 with
  `B`, number type 2 with `N` (double narrowed to float), string type 3 with `S`; undefined, objects, arrays and clips
  become type 0. A missing path returns type 0.
* **GetVariableBool / Number / String:** request Scaleform's conversion (boolean `ToBoolean`, number, string conversion;
  Scaleform, not read), so a string variable `"0"` as bool or a number variable as string works as ActionScript would. A
  missing path returns false, 0 or the empty string. (When the movie is absent the bool and number natives return an
  unspecified register value; implement as false and 0.)
* **GetVariableObject(Path, Type):** see wrapper objects; `None` for a missing path or movie.
* **Array variants (`Get/SetVariableArray`, `...IntArray`, `...FloatArray`, `...StringArray`):** not read.
* **Implementer checklist:** map to the adapter's `get`/`set`/`apply`; strings are UTF-16 in script, convert to JS strings;
  numbers are doubles; keep the missing-path results above; do not translate strings.

## CreateObject, CreateArray

* **CreateObject(ASClass, Type):** creates a new ActionScript object of the named class (by its ActionScript class path;
  the constructor is called with no arguments) in the movie, and wraps it (class `Type`, default `GFxObject`). A missing movie or a
  failed creation gives `None`.
* **CreateArray():** creates an empty ActionScript array and wraps it as a `GFxObject`.

## Start, Advance, SetPause, Close, GetPC, GetLP

* **Start(StartPaused):** `GearboxGFxMovie.Start` is script: it sets the mouse flag, calls `InitMoviePlayback`, calls the
  native `GFxMoviePlayer.Start`, then `MyDefinition.PostMovieStart(Self)` on success, else `ShutdownMoviePlayback`; the
  native is a virtual that the game's own movie class does not override. Native behaviour: if a movie is already
  loaded it only applies the pause flag (paused when `StartPaused` is true) and marks the player started, returning true.
  Otherwise it needs `MovieInfo` (the `SwfMovie`); it builds the object path of that movie (`Package.Name` or
  `Package.Group.Name`) and loads it; on failure it returns false and nothing else changes; on success it applies
  the pause flag, marks the player started, installs the three `_global` hooks above, applies the widget and data-store
  bindings, and returns true.
* **Advance(Time):** if loaded, advances the movie by `Time` seconds (Scaleform advance with the engine's frame mode) and
  then runs the post-advance step: if the `OnPostAdvance` delegate is bound it is called with `DeltaTime`, and if a CLIK
  widget has initialised since the last advance the script event `PostWidgetInit` is raised once and the flag cleared.
  Unloaded movie: nothing. Synchronous; ActionScript timeline code runs inside it.
* **SetPause(bPausePlayback):** stores "playing" as the negation of the argument. The default for an omitted argument was
  not established (the engine's public declaration is true; treat an omitted argument as true).
* **Close(Unload):** needs the manager and a loaded movie. In order: the script event `OnClose` is raised (unless the
  player is a class default object); with `Unload` true the player is taken off the manager's active list; the script event
  `OnCleanup` is raised; the movie is stopped (with `Unload` it is destroyed); the started flag is cleared; the script event
  `ConditionalClearPause` is raised; the manager recomputes input focus. With `Unload` true the player then calls its
  cleanup on every registered wrapper (they become inert), empties the set, clears its movie pointer and flags itself as
  closed. `GearboxGFxMovie.OnClose` then fires the `OnClosed` delegate and `ShutdownMoviePlayback` (script). `Close()` with the
  argument omitted passes false. After an unloading `Close` all `GFxObject` natives on that player are no-ops.
* **GetLP():** the local player at `LocalPlayerOwnerIndex` in `Engine.GamePlayers` (a negative index is reset to 0; out of
  range gives `None`). **GetPC():** that local player's `Actor` (the player controller), `None` when there is no local
  player. These are pure lookups; the accept screen's script casts the result to `WillowPlayerController`.
* **RegisterGFxObject / UnregisterGFxObject:** add to or remove from the player's wrapper set; no ActionScript effect.

## GFxObject members: Get/Set, text, translation

* **Get / GetBool / GetFloat / GetString / GetObject:** read member `Member` of this object (object, array or clip only;
  else the zero value). `GetString` asks for the string conversion and returns "" when the value is neither string kind
  after conversion (undefined, missing). `GetObject` wraps with `Type` (default `GFxObject`). `Get` returns an `ASValue`
  like `GetVariable`.
* **Set / SetBool / SetFloat / SetObject:** set a member (object, array or clip only; else nothing).
* **SetString(Member, S, InContext):** first translates `S` (below), then stores the result as a wide string in the member.
* **SetText(Text, InContext):** translates `Text`; then, only when this wrapper is a **display object (clip or text
  field)**: if the translation applied at least one font or colour tag and met no unknown tag, the translated text is stored
  into the member `htmlText`; otherwise the text is set as plain text (`text`) with any unknown tag left in it verbatim.
  Other wrapper kinds: nothing.
* **TranslateString(StringToTranslate, InContext):** returns the translated string.
* **Translation rules (the engine's text markup):** the string is cut into tag and text pieces; a piece starting with
  `<` is a tag. Recognised tags: `<Font:Name>` becomes `<FONT FACE='Name'>`; `<Color:R=r,G=g,B=b,A=a>` (floats, each channel
  read after its `R=`, `G=`, `B=`, `A=` label) becomes `<FONT COLOR='#RRGGBB'>` with each channel computed as
  `int(value * 255 * 2 + 0.5) >> 1` clamped to 0..255 (round half up; alpha is parsed, defaults to 1 and is ignored);
  `</Font` and `</Color` (and `</FONT>`) become `</FONT>`. A tag of the form `<Name:Argument/>` is looked up among
  the `TranslatorTags` of the given `TranslationContext` and then among the engine's default translators; a hit is
  replaced by the translator's output (itself translated again). Any other tag is copied unchanged and marks the string as
  "has unknown markup". A line break piece is copied as is. A colour tag with a missing label also marks unknown markup.
* **Implementer checklist:** keep the display-object rule for `SetText` (html only when conversions happened and nothing
  unknown); implement Font and Color as above; leave translator-tag lookup as a hook (the mission text does not need it).
* **Open:** the exact tokeniser for nested `<` characters in the argument; the default translators' names.

## GearboxGFxMovie

* **PlayUISound(UIEvent):** a virtual: looks the event name up first in this movie's `InteractionOverrideSounds` list (name
  to sound event pairs), then in the UI manager's list of the same shape; when found, posts that audio event as a 2D UI
  sound; when not found nothing happens (no error). `PlaySpecialUISound(string)` is script and just converts the string to a
  name and calls it. The accept screen uses `MenuOpen`, `MenuClose`, `accept_mission`.
  Implementer: map the name through the same two tables; the host may map to an existing UI sound event or ignore it.
* **SingleArgInvokeS / F / B(Command, Arg):** invoke the ActionScript method `Command` once with a string, a float or a
  bool argument (the bool is normalised to true/false). `Command` is a full path in the movie (script builds
  `<clip path>.<method>`; the HUD calls it 54 times for strings). They go through the same path as
  `ActionScript*` with one explicit argument (the implementing helpers were not read in detail: medium confidence).
* **GetInstanceContextObject():** returns the object this movie was opened for: the movie's `ContextObject` field when it
  is set; otherwise it walks `PlayerOwners` and returns what the first owner that has a controller reports through a
  virtual call that was not followed; `None` when neither gives anything. The accept screen casts the result to
  `IMissionDirector` and `IFocusable` (NPC Marcus), so for the Fire mission it must be the Marcus pawn that
  `WillowPlayerController.ClientGFxPlayMovie` passes in (set `ContextObject`; lane C1 covers that call).
* **GetLocalPlayer():** the same lookup as `GFxMoviePlayer.GetLP`.
* **InitFromDefinition():** a virtual hook (the Gearbox movie class has an empty override in the packages that were read; the
  Willow movie class overrides it); `InitMoviePlayback` and `ShutdownMoviePlayback` are natives called from script and
  were not read.
* **GFxColoredText(C, S, bAppendNewline):** wraps `S` in the colour markup the translation above understands; not read.
* **Not read:** `MovieState_*`, `AddStyle`, `RemoveStyle*`, `GetRenderTexture`, `SetExternalTexture`, `SetMouseableFlag`,
  `IsShowingFlashMouse`, `WantsControllerInput`, `InitializeFrom`.

## WillowGFxMovie3D.FocusOn(Thing)

3D movies are drawn on a plane in the world and offset toward what is focused. `FocusOn(Thing)` (`Thing` a `GFxObject`)
first makes sure the clip (and, through `GetObject("_parent")`, its parent clip) is known to the movie's clip tables
(`ChainedObjectMap`, `RedundantClipArray`), stores `Thing` in `FocusObject`, and recomputes the 3D focus offset: the centre of the target's
bounding rectangle in movie coordinates minus half the movie size, limited to +/-100000. It has no ActionScript side
effect. The accept screen calls it from `SetFocus` with the object at `missions.selections.selection<N>`, where
`N = HighlightedEntry - IndexOfTopEntry + 1` (a number appended to the literal path; read from script).
Low-medium confidence on the offset arithmetic. **Implementer:** a no-op is safe for the flat Ruffle presentation; if the
host moves the panel, use the formula above and keep `None` tolerance.

## QuestAcceptGFxMovie.UpdateMissionTextList

* **Signature:** `native function UpdateMissionTextList()`; a virtual of the movie class. No script caller appears in
  `QuestAcceptGFxMovie`; it is raised from elsewhere (open item).
* **Reads:** the movie's `MissionTextList` (a `GFxTextListContainer`), its `MissionList` (array of
  `StatusMenuMissionEligibilityData`: `MissionDef`, `MissionStatus`), its own `MissionCategories` (array of strings), and the
  localisation section `WillowGame` / `CategoryLabels`.
* **Does (in order):** nothing at all if `MissionTextList` is `None`. Otherwise: empties `MissionCategories`; for every
  entry in the list's sortable array (the data entries, in their current order) it finds the entry's mission, looks it up in
  the movie's `MissionList` by `MissionDef`, and reads its `MissionStatus`; status **0 (available)** yields the localised
  label `CategoryLabels` key `mlistavailable`, status **3 (ready to turn in)** the key `mlistturnin` (package `WillowGame`);
  any other status (1, 2: in progress) or a mission not in `MissionList` yields "no category" (index -1). A label is
  added to `MissionCategories` if no case-insensitive equal label is there already, and the entry's category index is the
  label's index. Then the list rebuilds its displayed `TextEntries`: for each non-filtered data entry, when its category
  index differs from the previous displayed entry's and is a valid index, a category-header row is emitted before it, then
  the entry's own row; and the list's refresh virtual is called (the list redraws through `ActionScript*`).
* **Calls other natives:** `GFxTextListContainer` rebuild (native), `Localize` for the category labels (see
  [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md), `Object.Localize`).
* **Edge cases:** entries whose mission is not in `MissionList` get no header; a list with only in-progress missions has
  no headers; headers appear only where the category changes, so entries must already be ordered by category (the order
  comes from `DetermineQuestEntries`: redeemable first, then eligible, then in progress).
* **Implementer checklist:** the output is the ordered list of rows (header or mission) the movie's list shows; for the Fire
  mission offered by Marcus (status 0) the list shows one header "available" (the `mlistavailable` string) then the mission
  title (`MissionDefinition.MissionName`); after it is accepted the mission is in progress and has no header, and when ready
  it sits under the turn-in header.
* **Open:** the exact English strings of the two keys (from `WillowGame.int`, not read); the origin of the call.

## Not read yet

`GFxObject.GetElement*`, `SetElement*` (array element accessors, same conversions expected), `CreateEmptyMovieClip`,
`AttachMovie`, `GotoAndPlay/Stop`, `SetVisible`, `SetPosition`, `Get/SetDisplayInfo`, `Get/SetDisplayMatrix`,
`Get/SetColorTransform`, `GetText`, `GFxObject.Invoke` (expected to match the movie-level one with this object as `this`),
`GetVariableArray` family, `SetViewport`, `SetViewScaleMode`, `SetAlignment`, `SetTimingMode`, `SetPriority`,
input natives (`InputKey`, `AddCaptureKey`, `WantsInput`...), `SetExternalTexture`, the data-store publish/refresh pair,
`WillowGFxMovie3D` sliding-object and split-screen helpers, `GearboxGFxMovie.MovieState_*` and styles, the `GFxTextListContainer`
natives other than the rebuild used above, `WillowHUDGFxMovie` (88 natives, the HUD mission widget), the reward screen's
`WillowGFxMovie` natives, and the meaning of the `fscommand` return value.

## Corrections to earlier notes

None to earlier notes. For the census ([NATIVE_SLICE_CENSUS.md](NATIVE_SLICE_CENSUS.md)): `GFxMoviePlayer.ActionScript` is the
same native as `ActionScriptVoid`, `ActionScriptInt` and `ActionScriptFloat` share one implementation, as do
`GFxObject.ActionScriptObject` and `ActionScriptArray`. Every script call to an `ActionScript*` native passes only a path string, yet each one sends the calling function's
parameters, so the census's call-site view hides the real arguments.
