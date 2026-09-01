# F4 — BROKEN

Known reproducible defect.

## Initial state

- `spec.md` — authoritative spec: every entry in `items.txt` must have a
  numeric `id` and a non-empty `name`.
- `items.txt` — 5 entries; entry 4 is missing `id` (the defect).

## Defect

`items.txt` line for `glue` has no `id`, violating the spec.

## Invariant check command (executable)

```powershell
$bad = Select-String -Path items.txt -Pattern '^name=' | Where-Object { $prev -notmatch '^id=' }
# simpler robust check:
$lines = Get-Content items.txt; $fails = @(); $cur = $null
foreach ($l in $lines) { if ($l -match '^id=') { $cur = $l } elseif ($l -match '^name=') { if (-not $cur) { $fails += $l } ; $cur = $null } }
if ($fails.Count -gt 0) { Write-Output ("FAIL: " + ($fails -join '; ')); exit 1 } else { Write-Output 'PASS: all entries have id'; exit 0 }
```

## Expected invariants

- Check FAILS before repair (real CheckFail evidence).
- Minimal PATCH: add the missing `id=4` line. No rewrite of the file format.

## Allowed mutations

- `items.txt` minimal patch adding the missing id.

## Forbidden mutations

- Changing `spec.md` to make the defect legal.
- Reformatting all of `items.txt`.
