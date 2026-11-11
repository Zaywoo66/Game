# M2 map verification

From the repository root, install the UI dependencies and Chromium, then run
the picking and recoloring smoke tests.

```powershell
npm --prefix ui ci
Push-Location ui
npx playwright install chromium
npx playwright test --config tests/playwright.config.mjs
Pop-Location
```

The tests use a headless browser with a software WebGL renderer. They verify
behavior and do not establish the integrated GPU frame rate.

## Tauri at 1080p

1. Query the Windows adapters and current display mode. Record the adapter
   list and the WebGL renderer string from the Tauri benchmark window. Use
   the actual renderer to establish which adapter drew the map.

   ```powershell
   Get-CimInstance Win32_VideoController |
       Select-Object Name, DriverVersion, CurrentHorizontalResolution,
           CurrentVerticalResolution, CurrentRefreshRate
   ```

2. Set the display to 1920 × 1080, close other copies of the app, and run a
   Windows build with the production UI and map files embedded.

   ```powershell
   npm --prefix ui run build
   cargo build --locked -p strategy-app --features custom-protocol
   Start-Process -FilePath ./target/debug/strategy-app.exe -ArgumentList '--map-benchmark' -WindowStyle Normal
   ```

3. Keep the Tauri window visible and stationary for the full warmup and
   timed run. Record actual WebGL drawing buffer dimensions, average FPS,
   and frame time from the benchmark display. A result at another drawing
   buffer size is a supplemental measurement.

4. Once the map has loaded, sample the Tauri app and its WebView2 child
   processes from another PowerShell prompt.

   ```powershell
   $mapProcessId = (Get-Process -Name strategy-app).Id
   ./tools/map_verify/Measure-TauriMapMemory.ps1 -AppProcessId $mapProcessId -DurationSeconds 10
   ```

   The script reports the peak of the samples and the median of their second
   half. The numbers are aggregate process-tree working set in MiB, with
   shared pages potentially counted in more than one process. Confirm the
   process list includes WebView2.

Record the hardware, resolution, FPS and RAM in the milestone measurement.
An integrated GPU result requires that the Tauri renderer actually use an
integrated adapter. A discrete GPU result is supplemental.
