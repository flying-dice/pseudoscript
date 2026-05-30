# Install script for pds (PseudoScript, Windows).
# Usage: irm https://raw.githubusercontent.com/flying-dice/pseudoscript/main/scripts/install.ps1 | iex
#
# Environment variables:
#   PDS_INSTALL_DIR  - where to install (default: $env:USERPROFILE\.pseudoscript\bin)
#   PDS_VERSION      - version tag to install (default: latest)

$ErrorActionPreference = "Stop"

# Windows PowerShell 5.1 defaults to TLS 1.0; GitHub requires TLS 1.2+.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

function Fail($Message) {
    [Console]::Error.WriteLine("error: $Message")
    exit 1
}

$Repo = "flying-dice/pseudoscript"
$Binary = "pds.exe"

$InstallDir = if ($env:PDS_INSTALL_DIR) {
    $env:PDS_INSTALL_DIR
} else {
    Join-Path $env:USERPROFILE ".pseudoscript\bin"
}

$Version = if ($env:PDS_VERSION) {
    $env:PDS_VERSION
} else {
    "latest"
}

function Resolve-Version {
    if ($Version -eq "latest") {
        try {
            $release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
        } catch {
            Fail "Could not resolve latest version - are there any releases for $Repo yet?"
        }
        if (-not $release.tag_name) {
            Fail "Could not resolve latest version"
        }
        return $release.tag_name
    }
    return $Version
}

function Get-Target {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64"   { return "x86_64-pc-windows-msvc" }
        "Arm64" {
            Fail "No prebuilt binary for aarch64 Windows - build from source: cargo install --git https://github.com/$Repo"
        }
        default {
            Fail "Unsupported architecture: $arch"
        }
    }
}

# Verify a downloaded artifact against the release's SHA256SUMS asset.
function Test-Checksum($File, $Name, $ReleaseVersion, $TmpDir) {
    Write-Host "  Verifying checksum ..."

    $sumsUrl = "https://github.com/$Repo/releases/download/$ReleaseVersion/SHA256SUMS"
    $sumsPath = Join-Path $TmpDir "SHA256SUMS"

    try {
        Invoke-WebRequest -Uri $sumsUrl -OutFile $sumsPath -UseBasicParsing
    } catch {
        Fail "Could not download SHA256SUMS for $ReleaseVersion"
    }

    $expected = $null
    foreach ($line in Get-Content $sumsPath) {
        $parts = $line -split '\s+', 2
        if ($parts.Count -eq 2 -and $parts[1].Trim() -eq $Name) {
            $expected = $parts[0].Trim()
            break
        }
    }
    if (-not $expected) {
        Fail "No checksum listed for $Name"
    }

    # -ne on strings is case-insensitive; Get-FileHash returns uppercase hex.
    $actual = (Get-FileHash -Path $File -Algorithm SHA256).Hash
    if ($actual -ne $expected) {
        Fail "Checksum mismatch for $Name`n  expected: $expected`n  actual:   $actual"
    }
}

function Install-Pds {
    Write-Host "Installing pds..."

    $target = Get-Target
    $resolvedVersion = Resolve-Version
    $artifactName = "pds-$target"

    Write-Host "  Platform: $target"
    Write-Host "  Version:  $resolvedVersion"
    Write-Host "  Install:  $InstallDir"

    $downloadUrl = "https://github.com/$Repo/releases/download/$resolvedVersion/$artifactName.zip"

    $tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    try {
        $archivePath = Join-Path $tmpDir "archive.zip"
        Write-Host "  Downloading $downloadUrl ..."

        try {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath -UseBasicParsing
        } catch {
            Fail "Download failed - check that version '$resolvedVersion' exists and has a release asset for '$target'"
        }

        Test-Checksum -File $archivePath -Name "$artifactName.zip" -ReleaseVersion $resolvedVersion -TmpDir $tmpDir

        Expand-Archive -Path $archivePath -DestinationPath $tmpDir -Force

        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        $src = Join-Path $tmpDir $Binary
        $dest = Join-Path $InstallDir $Binary
        Move-Item -Path $src -Destination $dest -Force

        Write-Host ""
        Write-Host "pds $resolvedVersion installed to $dest"

        Write-PathHint
    } finally {
        Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Print PATH setup guidance, unless InstallDir is already on the user PATH.
function Write-PathHint {
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -like "*$InstallDir*") {
        return
    }
    Write-Host ""
    Write-Host "Add to your PATH (current session):"
    Write-Host "  `$env:PATH = `"$InstallDir;`$env:PATH`""
    Write-Host ""
    Write-Host "Add permanently:"
    Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `"$InstallDir;`$([Environment]::GetEnvironmentVariable('PATH', 'User'))`", 'User')"
}

Install-Pds
