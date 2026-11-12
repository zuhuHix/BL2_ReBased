## 2026-11-06: the material setter's result means nothing; read the value back

AI-assisted (Claude). The first editor run of the checked setter failed every row: 0 of 18 bound. In UE 5.8, `UMaterialEditingLibrary::SetMaterialInstanceTextureParameterValue` declares `bool bResult = false`, never sets it and returns it (read in the installed engine's `MaterialEditingLibrary.cpp`), so it returns false whether or not it worked. **The test's fake returned true, so the tests passed on a check that could never pass.** That fake assumed the API's meaning, and only a real run could show it. The step now checks that the parent has the parameter, then reads the value back and compares it; the fake returns false always, as the engine does.

- **Verified:** that the run touched no mesh: every mesh asset still references the earlier run's instance, with its time unchanged, and no new instance reached disk. Synthetic tests.
- **`UNVERIFIED`:** that the read-back holds in the editor, until the rerun.

Also from Luna's third review: each `test_interactives.ps1` run now has its own log and frames folder, and two balances sharing one frame name fail the check.
