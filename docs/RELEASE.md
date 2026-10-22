# Windows release procedure

## Qualification

1. Run `npm ci`, `npm run check`, `npm test`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
2. Exercise the Windows 10 and Windows 11 hardware matrix in `docs/ROADMAP.md`, including UAC denial and rollback timeout.
3. Review `npm audit` and `cargo audit` (when installed), generate an SBOM, and review new dependencies.
4. Build on a controlled x64 Windows signing host with `npm run tauri build`.

## Signing

The development artifacts are unsigned. For public distribution, use an organization-owned EV or standard Authenticode certificate from a trusted CA. Protect the key with a hardware token or managed signing service; never store it in the repository.

Sign the helper and application before packaging, then sign both installers. Example placeholders:

```powershell
signtool sign /fd SHA256 /tr https://timestamp.example /td SHA256 /sha1 <CERT_THUMBPRINT> target\release\windows-helper.exe
signtool sign /fd SHA256 /tr https://timestamp.example /td SHA256 /sha1 <CERT_THUMBPRINT> target\release\charroutex.exe
npm run tauri build
signtool sign /fd SHA256 /tr https://timestamp.example /td SHA256 /sha1 <CERT_THUMBPRINT> target\release\bundle\msi\CharRouteX_1.1.0_x64_en-US.msi
signtool sign /fd SHA256 /tr https://timestamp.example /td SHA256 /sha1 <CERT_THUMBPRINT> target\release\bundle\nsis\CharRouteX_1.1.0_x64-setup.exe
```

Replace the timestamp URL and thumbprint with approved organizational values. Verify every binary with `signtool verify /pa /all /v <path>` on a clean machine.

## Distribution checklist

- Record SHA-256 hashes for MSI, NSIS, application, and helper.
- Install, upgrade, repair, and uninstall both installer formats in clean VMs.
- Confirm WebView2 bootstrap behavior, Start menu entries, resource helper path, UAC publisher, database upgrade, and rollback after closing the UI.
- Publish privacy disclosure, probe destinations, support diagnostics, release notes, hashes, and minimum OS requirements together.
