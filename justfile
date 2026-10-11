# Development tasks. `just` lists them; `just dev` or `just tauri` is the usual start.
#
# The frontend (web/README.md) runs against a daemon, and `data` picks which:
#
#   mock  the real `seula --server` over the database `mockup/data/generate.py` seeds, on
#         ports off the real defaults so a running daemon does not collide (the default)
#   real  `seula --server` with your own config.toml, so your own projects; the database
#         starts empty until the app's Scan button fills it from the configured folders

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]
# For `[script]`, unstable before just 1.44.
set unstable

python := if os_family() == "windows" { "python" } else { "python3" }
exe := if os_family() == "windows" { ".exe" } else { "" }
mock_db := "mockup/data/seula-mock.db"
mock_config := "mockup/data/mock-config.toml"
# generate.py's HTTP_PORT. Not the frontend's DEFAULT_URL (web/shared/api.ts), which is the
# real port: the recipes below pass this one in VITE_SEULA_URL.
mock_url := "http://127.0.0.1:50152"
# The default http_port. A config.toml that sets another one needs this changed too.
real_url := "http://127.0.0.1:50052"

[private]
default:
    @just --list

# The frontend in a browser at http://localhost:1420; `just dev real` for your own projects
dev data="mock": (_ready data) (_with-daemon "dev" data)

# The frontend in the Tauri shell; `just tauri real` for your own projects
tauri data="mock": (_ready data) (_with-daemon "tauri dev" data)

# Only the daemon, in the foreground, for running it in its own terminal
[windows]
daemon data="mock": (_ready data)
    Get-ChildItem env:SEULA_* | Remove-Item; target/debug/seula{{exe}} {{ if data == "mock" { "--config " + mock_config + " " } else { "" } }}--server

# Only the daemon, in the foreground, for running it in its own terminal
[unix]
[script("bash")]
daemon data="mock": (_ready data)
    unset ${!SEULA_@}
    exec target/debug/seula{{exe}} {{ if data == "mock" { "--config " + mock_config + " " } else { "" } }}--server

# Seed the mock database, write its config, install the frontend's packages; skips what exists
setup: _mock-db _mock-config _npm

# Rebuild the mock database from scratch, discarding edits made through the UI (stop the daemon first)
reseed:
    {{python}} mockup/data/generate.py seed
    {{python}} mockup/data/generate.py config

# Build the daemon
build:
    cargo build --bin seula

# Build the Windows installer (ADR-0068): web/target/release/bundle/nsis/*-setup.exe
#
# The daemon and the scanner worker are built apart on purpose. Built together, Cargo
# unifies vst-meta's features and links the VST hosts into the daemon (ADR-0004). Both
# get a static C runtime so the installer needs no Visual C++ redistributable.
[windows]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
release: _npm
    $ErrorActionPreference = 'Stop'
    function Run { param([string]$what, [scriptblock]$cmd) & $cmd; if ($LASTEXITCODE -ne 0) { throw "$what failed ($LASTEXITCODE)" } }
    $triple = ((rustc -vV) -match '^host: (.+)$' | Select-Object -First 1) -replace '^host: ', ''
    if (-not $triple) { throw 'could not read the host triple from rustc -vV' }
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    Run 'daemon build' { cargo build --release -p seula }
    Run 'worker build' { cargo build --release -p vst-meta }
    $bin = 'web/solid/src-tauri/binaries'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item target/release/seula.exe "$bin/seula-$triple.exe" -Force
    Copy-Item target/release/vst-meta.exe "$bin/vst-meta-$triple.exe" -Force
    Set-Location web/solid
    Run 'tauri build' { npx tauri build --config src-tauri/tauri.release.conf.json }
    Get-ChildItem ../target/release/bundle/nsis/*-setup.exe | ForEach-Object { Write-Host "$($_.FullName)  $([math]::Round($_.Length / 1MB, 1)) MB" }

# Networking is off. One sandbox runs at a time, so close an open one first; this one is left
# open after it finishes, close it yourself. The generated .wsb and the results are under
# target/sandbox.
#
# Smoke-test the built installer (`just release` first) in a clean Windows Sandbox
[windows]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
smoke:
    $ErrorActionPreference = 'Stop'
    if (Get-Process -Name 'WindowsSandbox*' -ErrorAction SilentlyContinue) { Write-Host 'A Windows Sandbox is already open; close it first.'; exit 1 }
    $root = (Get-Location).Path
    $installer = "$root\web\target\release\bundle\nsis"
    if (-not (Get-ChildItem $installer -Filter '*-setup.exe' -ErrorAction SilentlyContinue)) { Write-Host 'No installer found; run `just release` first.'; exit 1 }
    $out = "$root\target\sandbox\out"
    if (Test-Path $out) { Remove-Item $out -Recurse -Force }
    New-Item -ItemType Directory -Force $out | Out-Null
    $wsb = "$root\target\sandbox\smoke.wsb"
    @"
    <Configuration>
      <Networking>Disable</Networking>
      <MappedFolders>
        <MappedFolder><HostFolder>$installer</HostFolder><SandboxFolder>C:\in</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
        <MappedFolder><HostFolder>$root\scripts</HostFolder><SandboxFolder>C:\scripts</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
        <MappedFolder><HostFolder>$out</HostFolder><SandboxFolder>C:\out</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
      </MappedFolders>
      <LogonCommand><Command>powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\scripts\smoke-installer.ps1</Command></LogonCommand>
    </Configuration>
    "@ | Set-Content $wsb -Encoding utf8
    Start-Process "$env:windir\System32\WindowsSandbox.exe" -ArgumentList "`"$wsb`""
    Write-Host 'Sandbox started; waiting for the result (up to 10 minutes)...'
    $result = "$out\smoke-result.txt"
    $done = $false
    for ($i = 0; $i -lt 120 -and -not $done; $i++) {
        Start-Sleep -Seconds 5
        $text = try { Get-Content $result -Raw -ErrorAction Stop } catch { '' }
        if ($text -match 'ALL CHECKS PASSED|CHECK\(S\) FAILED') { $done = $true }
    }
    if (-not $done) { Write-Host "No result after 10 minutes; see $out"; exit 1 }
    Write-Host $text
    if ($text -match 'CHECK\(S\) FAILED') { exit 1 }

# Networking is off. Two small folders of Live sets sit on the sandbox's Desktop, to add as
# project folders.
#
# Open the built installer (`just release` first) in an interactive Windows Sandbox
[windows]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
sandbox:
    $ErrorActionPreference = 'Stop'
    if (Get-Process -Name 'WindowsSandbox*' -ErrorAction SilentlyContinue) { Write-Host 'A Windows Sandbox is already open; close it first.'; exit 1 }
    $root = (Get-Location).Path
    $installer = "$root\web\target\release\bundle\nsis"
    if (-not (Get-ChildItem $installer -Filter '*-setup.exe' -ErrorAction SilentlyContinue)) { Write-Host 'No installer found; run `just release` first.'; exit 1 }
    # Two folders of three minimal sets each (EnumEvent 201 is 4/4, how the parser reads the time signature)
    $sets = "$root\target\sandbox\Live Sets"
    $xml = '<?xml version="1.0" encoding="UTF-8"?><Ableton MajorVersion="5" MinorVersion="12.0_12049" SchemaChangeCount="7" Creator="Ableton Live 12.0"><LiveSet><Tempo><Manual Value="128.0"/></Tempo><EnumEvent Id="0" Value="201"/></LiveSet></Ableton>'
    foreach ($set in 'Set A', 'Set B') {
        $d = Join-Path $sets $set
        New-Item -ItemType Directory -Force $d | Out-Null
        foreach ($n in 1..3) {
            $fs = [IO.File]::Create((Join-Path $d "Song $n.als"))
            $gz = New-Object IO.Compression.GZipStream($fs, [IO.Compression.CompressionMode]::Compress)
            $b = [Text.Encoding]::UTF8.GetBytes($xml); $gz.Write($b, 0, $b.Length); $gz.Dispose(); $fs.Dispose()
        }
    }
    $wsb = "$root\target\sandbox\manual.wsb"
    @"
    <Configuration>
      <Networking>Disable</Networking>
      <MappedFolders>
        <MappedFolder><HostFolder>$installer</HostFolder><SandboxFolder>C:\Users\WDAGUtilityAccount\Desktop\installer</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
        <MappedFolder><HostFolder>$sets</HostFolder><SandboxFolder>C:\Users\WDAGUtilityAccount\Desktop\Live Sets</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
      </MappedFolders>
    </Configuration>
    "@ | Set-Content $wsb -Encoding utf8
    Start-Process "$env:windir\System32\WindowsSandbox.exe" -ArgumentList "`"$wsb`""
    Write-Host 'Sandbox started: the installer and the Live Sets are on its Desktop.'

# Typecheck and lint the frontend
check: _npm
    cd web/solid; npm run build
    cd web/solid; npm run lint

# Log a bug in docs/bugs.md: `just bug search-cap "Search stops at 200 rows"` (ADR-0065)
[windows]
[positional-arguments]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
bug name +description:
    $ErrorActionPreference = 'Stop'
    $name = $args[0]
    $text = ($args[1..($args.Count - 1)] -join ' ').Trim()
    $file = 'docs/bugs.md'
    if ($name -cnotmatch '^[a-z0-9]+(-[a-z0-9]+)*$') { Write-Host "the name is short kebab-case, like search-cap, not '$name'"; exit 1 }
    if (-not $text) { Write-Host 'a bug needs a description'; exit 1 }
    if (Select-String -Path $file -SimpleMatch -Pattern "``$name``" -Quiet) { Write-Host "``$name`` is already in $file; pick another name"; exit 1 }
    $line = "- [ ] $(Get-Date -Format 'yyyy-MM-dd') ``$name`` $text"
    $bytes = [IO.File]::ReadAllBytes((Resolve-Path $file))
    $lead = if ($bytes.Length -and $bytes[-1] -ne 10) { "`n" } else { '' }
    [IO.File]::AppendAllText((Resolve-Path $file), "$lead$line`n", [Text.UTF8Encoding]::new($false))
    Write-Host $line

# Log a bug in docs/bugs.md: `just bug search-cap "Search stops at 200 rows"` (ADR-0065)
[unix]
[positional-arguments]
[script("bash")]
bug name +description:
    set -euo pipefail
    name=$1; shift; text="$*"; file=docs/bugs.md
    [[ $name =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]] || { echo "the name is short kebab-case, like search-cap, not '$name'"; exit 1; }
    [[ -n ${text// } ]] || { echo 'a bug needs a description'; exit 1; }
    ! grep -qF "\`$name\`" "$file" || { echo "\`$name\` is already in $file; pick another name"; exit 1; }
    line="- [ ] $(date +%F) \`$name\` $text"
    [[ -z $(tail -c1 "$file") ]] || echo >> "$file"
    echo "$line" >> "$file"
    echo "$line"

# The open bugs in docs/bugs.md
bugs:
    @git grep --no-index -h -e '^- \[ \]' -- docs/bugs.md; exit 0

# Listed first so a mistyped `data` fails before anything is built.
[private]
_ready data: (_data data) _npm build

[private]
_data data:
    {{ if data == "mock" { "just _mock-db _mock-config" } else if data == "real" { "" } else { error("data is `mock` or `real`, not `" + data + "`") } }}

[private]
_mock-db:
    {{ if path_exists(mock_db) == "true" { "" } else { python + " mockup/data/generate.py seed" } }}

# Always rewritten: it holds absolute paths, so a moved checkout would point elsewhere.
[private]
_mock-config:
    {{python}} mockup/data/generate.py config

[private]
_npm:
    {{ if path_exists("web/solid/node_modules") == "true" { "" } else { "cd web/solid; npm install" } }}

# Start the daemon unless one is already answering, wait until it does, run
# `npm run <cmd>` in web/solid pointed at it, and stop the daemon again however that
# ends. SEULA_* variables are cleared because they override the config: for mock,
# SEULA_DATABASE_PATH would point the daemon at the real database, and for either, a port
# override would move it off the URL waited on.
[private]
[windows]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
_with-daemon cmd data:
    $ErrorActionPreference = 'Stop'
    # Vite's port (strictPort, vite.config.ts). Usually an earlier `just dev` still running.
    $vite = Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($vite) {
        $holder = Get-CimInstance Win32_Process -Filter "ProcessId=$($vite.OwningProcess)"
        Write-Host "port 1420 is taken by pid $($holder.ProcessId): $($holder.CommandLine)"
        Write-Host 'stop that first (an earlier `just dev` or `just tauri`?)'
        exit 1
    }
    $url = '{{ if data == "mock" { mock_url } else { real_url } }}'
    $daemonArgs = @({{ if data == "mock" { "'--config', '" + mock_config + "', " } else { "" } }}'--server')
    function Up { try { $null = Invoke-WebRequest "$url/health" -UseBasicParsing -TimeoutSec 1; $true } catch { $false } }
    $daemon = $null
    try {
        if (Up) {
            Write-Host "seula: using the daemon already on $url"
        } else {
            Get-ChildItem env:SEULA_* | Remove-Item
            $daemon = Start-Process 'target/debug/seula{{exe}}' -ArgumentList $daemonArgs -NoNewWindow -PassThru
            $null = $daemon.Handle  # keeps ExitCode readable after the process exits
            $deadline = (Get-Date).AddSeconds(30)
            while (-not (Up)) {
                if ($daemon.HasExited) { throw "seula --server exited with $($daemon.ExitCode)" }
                if ((Get-Date) -gt $deadline) { throw "seula --server did not answer on $url within 30 s" }
                Start-Sleep -Milliseconds 200
            }
        }
        $env:VITE_SEULA_URL = $url
        Set-Location web/solid
        npm run {{cmd}}
        exit $LASTEXITCODE
    } finally {
        if ($daemon -and -not $daemon.HasExited) { Stop-Process -Id $daemon.Id }
    }

[private]
[unix]
[script("bash")]
_with-daemon cmd data:
    set -euo pipefail
    # Vite's port (strictPort, vite.config.ts). Usually an earlier `just dev` still running.
    if curl -s -m 1 -o /dev/null http://localhost:1420; then
        echo 'port 1420 is taken; stop that first (an earlier `just dev` or `just tauri`?)' >&2
        exit 1
    fi
    url='{{ if data == "mock" { mock_url } else { real_url } }}'
    up() { curl -fs -m 1 "$url/health" > /dev/null; }
    if up; then
        echo "seula: using the daemon already on $url"
    else
        (unset ${!SEULA_@}; exec target/debug/seula{{exe}} {{ if data == "mock" { "--config '" + mock_config + "' " } else { "" } }}--server) &
        daemon=$!
        trap 'kill $daemon 2> /dev/null || true' EXIT
        for _ in $(seq 150); do
            up && break
            kill -0 $daemon 2> /dev/null || { echo 'seula --server exited' >&2; exit 1; }
            sleep 0.2
        done
        up || { echo "seula --server did not answer on $url within 30 s" >&2; exit 1; }
    fi
    cd web/solid
    VITE_SEULA_URL="$url" npm run {{cmd}}
