param(
    [Parameter(Mandatory=$true)]
    [string]$ExampleName,

    [Parameter(Mandatory=$false)]
    [string]$Profile = "release", # Or "debug"

    # CEF Source Path is now determined internally by default
    # [Parameter(Mandatory=$false)]
    # [string]$CefSourcePath = $env:CEF_PATH, # Assumes CEF_PATH is set per README

    # Relative path from this script (in cefpuppy) to the cargo target directory
    [Parameter(Mandatory=$false)]
    [string]$CargoTargetDir = "target",

    # Absolute or relative path from this script (in cefpuppy) for the final output
    [Parameter(Mandatory=$false)]
    [string]$FinalOutputDir = "..\..\cef" # Defaults to c:\GIT\petplay\cef
)

# --- 1. Setup Environment and Paths ---

Write-Host "Setting up environment..."

# Determine and set CEF Source Path internally
$CefSourcePath = Join-Path $env:USERPROFILE ".local\share\cef"
Write-Host "Setting CEF_PATH to default: $CefSourcePath"

# Validate CEF Source Path
if (-not (Test-Path $CefSourcePath)) {
    Write-Error "Default CEF Source Path not found: '$CefSourcePath'. Please ensure CEF is exported using 'cargo run -p export-cef-dir -- --force `$env:USERPROFILE/.local/share/cef' or provide a valid path."
    exit 1
}
# Set CEF_PATH for this script's scope (might influence cargo build if not already set globally)
$env:CEF_PATH = $CefSourcePath
Write-Host "Using CEF_PATH: $env:CEF_PATH"

# Determine CEF Binary directory (might be Release subdir on Windows)
$CefBinDir = $CefSourcePath
if (Test-Path (Join-Path $CefSourcePath "Release")) {
    $CefBinDir = Join-Path $CefSourcePath "Release"
    Write-Host "Using CEF binaries from: $CefBinDir"
} else {
     Write-Host "Using CEF binaries from: $CefBinDir"
}

# Add CEF binary dir to PATH for this script's scope (helps runtime loading if needed during build steps)
$env:PATH = "$env:PATH;$CefBinDir"
Write-Host "Temporarily added $CefBinDir to PATH"

# Calculate the build output directory within the target folder
# Corrected Join-Path: Nest calls or use -ChildPath with an array if needed for multiple segments
$BuildOutputDir = Join-Path -Path (Join-Path -Path (Join-Path -Path $PSScriptRoot -ChildPath $CargoTargetDir) -ChildPath $Profile) -ChildPath "examples"
# Example alternative using -ChildPath array (less common but works):
# $BuildOutputDir = Join-Path -Path $PSScriptRoot -ChildPath @($CargoTargetDir, $Profile, "examples")
Write-Host "Build output directory set to: $BuildOutputDir" # Added for verification

# Resolve the final output directory path
$FinalOutputFullPath = Join-Path $PSScriptRoot $FinalOutputDir # Resolve relative to script location
Write-Host "Final output will be placed in: $FinalOutputFullPath"


# --- 2. Build the Rust Example ---

Write-Host "Building example '$ExampleName' with profile '$Profile'..."
# Assuming Cargo.toml for examples is in the cefpuppy root (where the script is)
cargo build --profile $Profile --example $ExampleName
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed!"
    exit $LASTEXITCODE
}
Write-Host "Build successful."

# Verify the executable was created
$ExePath = Join-Path $BuildOutputDir "$ExampleName.exe"
if (-not (Test-Path $ExePath)) {
     Write-Error "Executable not found after build at $ExePath. Build might have failed silently or output is elsewhere."
     exit 1
}


# --- 3. Copy CEF Runtime Dependencies ---

Write-Host "Copying CEF runtime files to build output directory: $BuildOutputDir"

# Files and Directories to Copy
$FilesToCopy = @(
    "libcef.dll",
    "chrome_elf.dll",
    "v8_context_snapshot.bin",
    "d3dcompiler_47.dll",
    "vk_swiftshader.dll",
    "vulkan-1.dll",
    "resources.pak",
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
    "icudtl.dat",
    "dxcompiler.dll",
    "dxil.dll",
    "libEGL.dll",
    "libGLESv2.dll",
    "vk_swiftshader_icd.json"
)
$DirsToCopy = @(
    "locales"
)

# Ensure build output directory exists (cargo build should create it, but double-check)
if (-not (Test-Path $BuildOutputDir)) {
    New-Item -ItemType Directory -Path $BuildOutputDir -Force | Out-Null
}

# Copy Files
foreach ($file in $FilesToCopy) {
    $sourceFile = Join-Path $CefBinDir $file
    if (Test-Path $sourceFile) {
        # Write-Host "Copying $file..." # Optional: reduce verbosity
        Copy-Item -Path $sourceFile -Destination $BuildOutputDir -Force
    } else {
        Write-Warning "CEF source file not found: $sourceFile"
    }
}

# Copy Directories
foreach ($dir in $DirsToCopy) {
    $sourceDir = Join-Path $CefBinDir $dir
    if (Test-Path $sourceDir -PathType Container) {
         # Write-Host "Copying directory $dir..." # Optional: reduce verbosity
        Copy-Item -Path $sourceDir -Destination $BuildOutputDir -Recurse -Force
    } else {
        Write-Warning "CEF source directory not found: $sourceDir"
    }
}

# Copy Manifest if exists (specific to cefsimple example)
if ($ExampleName -eq "cefsimple") {
    $ManifestSource = Join-Path $PSScriptRoot "examples\cefsimple\win\cefsimple.exe.manifest" # Relative to script location
    if (Test-Path $ManifestSource) {
        # Write-Host "Copying manifest..." # Optional: reduce verbosity
        Copy-Item -Path $ManifestSource -Destination (Join-Path $BuildOutputDir "cefsimple.exe.manifest") -Force
    } else {
         Write-Warning "Manifest file not found: $ManifestSource"
    }
}
Write-Host "Dependency copying complete."


# --- 4. Move Build Output to Final Location ---

Write-Host "Moving build output from $BuildOutputDir to $FinalOutputFullPath"

# Ensure final directory exists and is empty (optional, depends on desired behavior)
if (Test-Path $FinalOutputFullPath) {
    Write-Host "Removing existing content in $FinalOutputFullPath..."
    Remove-Item -Path "$FinalOutputFullPath\*" -Recurse -Force
} else {
    Write-Host "Creating final output directory $FinalOutputFullPath..."
    New-Item -ItemType Directory -Path $FinalOutputFullPath -Force | Out-Null
}

# Move the contents
Write-Host "Moving files..."
Move-Item -Path "$BuildOutputDir\*" -Destination $FinalOutputFullPath -Force

# Optional: Clean up the now-empty build output directory
# Remove-Item -Path $BuildOutputDir -Recurse -Force

Write-Host "Build and packaging complete. Output is in $FinalOutputFullPath"
