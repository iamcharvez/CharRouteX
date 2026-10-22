# SOUTH COT INFRA migration guide

## Files

- Keep `SOUTH COT INFRA.kmz` as the untouched source.
- Upload `SOUTH COT INFRA - CLEANED.kmz` for migration.
- Use `SOUTH COT INFRA - MIGRATION AUDIT.csv` to resolve every manual-review row.
- Re-run `prepare_south_cot_migration.ps1` if the source KMZ changes.

## Expected cleaned preview

- 854 total features
- 752 point assets
- 102 fiber routes
- 0 empty geometries
- 0 external network links

The cleanup removed 71 exact logical duplicates, two empty routes, and one broken `Realtime GPS` link that referenced another computer. The original KMZ was not modified.

## Classified point assets

| Proposed category | Count |
| --- | ---: |
| Distribution point | 528 |
| NAP / NP | 155 |
| Closure / insertion | 40 |
| Node / POP | 5 |
| Local convergence point | 2 |
| Manual type review | 22 |

The proposed category is stored in each feature's `migration_type` ExtendedData field. Generic `DP`, blank, punctuation-only, and `Untitled Placemark` names were given stable `SC-*` names. Their original names remain in `source_name`.

## Fiber capacity

| Capacity | Route count |
| --- | ---: |
| 12 cores | 16 |
| 24 cores | 4 |
| 48 cores | 15 |
| Unknown — manual review | 66 |
| Mixed `48C TO 12C` — split/review | 1 |

Do not accept a default 1×1-core value for the 67 review routes unless field documentation confirms it. Update those routes from as-built plans, splice diagrams, OTDR records, or field verification.

## Import workflow

1. Replace the current upload with `SOUTH COT INFRA - CLEANED.kmz`.
2. Confirm the preview shows exactly 854 features: 752 points and 102 routes. Stop if the totals differ.
3. Paste the complete [`tools/dcgis_import_transform.py`](../tools/dcgis_import_transform.py) script into the import-page Python panel.
4. Do not use the built-in **Fiber routes** preset: it assigns every unknown route as 1×12. The flexible script maps the stated 12/24/48-core totals to logical DCGIS layouts and flags unknown capacity. Those layouts still require comparison with the manufacturer's physical buffer/tube schedule; the script writes that verification note onto each known-capacity route.
5. Do not apply a global sequential-name preset: the cleaned file already numbers generic points while preserving meaningful names.
6. Click **Preview transformation**. Preview must show no errors before import.
7. Import one test batch first. Inspect several NAPs, DPs, closures, and routes before importing the remaining features.
8. Filter the audit CSV by `manual_review_type`, `manual_review_capacity`, and `manual_review_split_capacity`; resolve all 89 review items.

## Recommended transformation

Use [`tools/dcgis_import_transform.py`](../tools/dcgis_import_transform.py). It uses DCGIS's validated type identifiers and supported fiber fields (`buffer_count` and `cores_per_buffer`), retains existing codes, adds deterministic codes only when missing, and appends idempotent review notes.

DCGIS supports `unknown`, `pole`, `client`, `nap`, `lcp`, `closure`, `fiber`, `node`, `outdoor_node`, `data_center`, `cabinet`, and `site`. It has no separate DP type, so the script exposes `MAP_DP_TO_NAP` as a configuration switch and documents every mapped DP in its notes.

## Acceptance checks

- No unexpected point appears outside the Philippines.
- No route has fewer than two vertices.
- No feature is imported twice at the same coordinates with the same name.
- Every route has a verified core count or an explicit `capacity unknown` status.
- The `PATH 48C TO 12C` feature is split at the capacity transition or documented as a mixed-capacity route.
- All 22 unclassified points receive a reviewed type.

