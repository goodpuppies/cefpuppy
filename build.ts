import { parseArgs } from "jsr:@std/cli/parse-args";
import * as path from "jsr:@std/path";
import * as fs from "jsr:@std/fs";

// Helper function to display usage
function showUsage() {
  console.log("\nUsage: deno run build.ts --example=<example_name> [options]");
  console.log("\nOptions:");
  console.log("  --example, -e                 Name of the example to build (required)");
  console.log("  --profile, -p                 Build profile (default: release)");
  console.log("  --cargoTargetDir, -c          Path to cargo target directory (default: target)");
  console.log("  --finalOutputDir, -f          Path for final output (default: ../../cef)");
  console.log("  --skipPdb, -s                 Skip PDB files to reduce size (default: true)");
  console.log("  --minLocales, -m              Only include en-US locale (default: true)");
  console.log("  --includeDxCompiler, -d       Include dxcompiler.dll (default: false)");
  console.log("  --useUpx, -u                  Compress binaries with UPX if available (default: true)");
  console.log("\nExample:");
  console.log("  deno run --allow-read --allow-write --allow-env --allow-run build.ts --example=cefsimple");
  console.log("  deno run --allow-read --allow-write --allow-env --allow-run build.ts -e cefsimple -p debug -s false");
  console.log("  deno run --allow-read --allow-write --allow-env --allow-run build.ts -ExampleName cefsimple -MinLocales false");
  Deno.exit(1);
}

// Print raw args for debugging
console.log("Raw arguments:", Deno.args);

// Special handling for PowerShell-style arguments which come in pairs
function processPowerShellArgs(args: string[]): Record<string, string> {
  const result: Record<string, string> = {};
  
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    // Check if this is a PowerShell-style flag (starts with - or --)
    if (arg.startsWith('-')) {
      const key = arg.replace(/^-+/, ''); // Remove leading dashes
      // Check if there's a value after this flag
      if (i + 1 < args.length && !args[i + 1].startsWith('-')) {
        result[key] = args[i + 1];
        i++; // Skip the next item as we've used it as a value
      } else {
        result[key] = 'true'; // Flag without value
      }
    }
  }
  
  console.log("Processed PowerShell args:", result);
  return result;
}

// Process PowerShell-style arguments
const psArgs = processPowerShellArgs(Deno.args);

// Try to get example name from PowerShell-style args first
let exampleName: string | undefined = psArgs.ExampleName || psArgs.example || psArgs.e;
let profile: string = psArgs.Profile || psArgs.profile || psArgs.p || "release";
let cargoTargetDir: string = psArgs.CargoTargetDir || psArgs.cargoTargetDir || psArgs.c || "target";
let finalOutputDir: string = psArgs.FinalOutputDir || psArgs.finalOutputDir || psArgs.f || "../../cef";
// New size optimization options
let skipPdb: boolean = psArgs.SkipPdb || psArgs.skipPdb || psArgs.s ? 
                         (psArgs.SkipPdb || psArgs.skipPdb || psArgs.s) === "true" : true;
let minLocales: boolean = psArgs.MinLocales || psArgs.minLocales || psArgs.m ? 
                            (psArgs.MinLocales || psArgs.minLocales || psArgs.m) === "true" : true;
let includeDxCompiler: boolean = psArgs.IncludeDxCompiler || psArgs.includeDxCompiler || psArgs.d ? 
                                  (psArgs.IncludeDxCompiler || psArgs.includeDxCompiler || psArgs.d) === "true" : true;
let useUpx: boolean = psArgs.UseUpx || psArgs.useUpx || psArgs.u ? 
                      (psArgs.UseUpx || psArgs.useUpx || psArgs.u) === "true" : true;

// Parse command-line arguments with standard parser as fallback
const parsedArgs = parseArgs(Deno.args, {
  string: ["example", "profile", "cargoTargetDir", "finalOutputDir"],
  boolean: ["skipPdb", "minLocales", "includeDxCompiler", "useUpx"],
  default: {
    profile: "release",
    cargoTargetDir: "target",
    finalOutputDir: "../../cef",
    skipPdb: true,
    minLocales: true,
    includeDxCompiler: false,
    useUpx: false
  },
  alias: {
    e: "example",
    p: "profile",
    c: "cargoTargetDir",
    f: "finalOutputDir",
    s: "skipPdb",
    m: "minLocales",
    d: "includeDxCompiler",
    u: "useUpx"
  }
});

// Use parsed args as fallback
exampleName = exampleName || parsedArgs.example;
profile = profile || parsedArgs.profile;
cargoTargetDir = cargoTargetDir || parsedArgs.cargoTargetDir;
finalOutputDir = finalOutputDir || parsedArgs.finalOutputDir;
skipPdb = parsedArgs.skipPdb !== undefined ? parsedArgs.skipPdb : skipPdb;
minLocales = parsedArgs.minLocales !== undefined ? parsedArgs.minLocales : minLocales;
includeDxCompiler = parsedArgs.includeDxCompiler !== undefined ? parsedArgs.includeDxCompiler : includeDxCompiler;
useUpx = parsedArgs.useUpx !== undefined ? parsedArgs.useUpx : useUpx;

// Validate required arguments
if (!exampleName) {
  console.error("Error: Missing required argument: example");
  showUsage();
}

// At this point, exampleName is guaranteed to be a string since showUsage() will exit if it's undefined
// We can use a type assertion to tell TypeScript that exampleName is a string
const example: string = exampleName as string;

// Main build function
async function buildExample() {
  console.log(`Using example: ${example}`);
  console.log(`Using profile: ${profile}`);
  console.log(`Size optimizations: skipPdb=${skipPdb}, minLocales=${minLocales}, includeDxCompiler=${includeDxCompiler}, useUpx=${useUpx}`);

  // --- 1. Setup Environment and Paths ---  
  
  console.log("Setting up environment...");
  
  // Determine script root directory
  const scriptDir = path.dirname(path.fromFileUrl(import.meta.url));
  
  // Determine and set CEF Source Path internally
  const homeDir = Deno.env.get("HOME") || Deno.env.get("USERPROFILE") || "";
  const cefSourcePath = path.join(homeDir, ".local", "share", "cef");
  console.log(`Setting CEF_PATH to default: ${cefSourcePath}`);
  
  // Validate CEF Source Path
  try {
    const cefPathInfo = await Deno.stat(cefSourcePath);
    if (!cefPathInfo.isDirectory) {
      console.error(`Default CEF Source Path exists but is not a directory: '${cefSourcePath}'`);
      Deno.exit(1);
    }
  } catch (error) {
    console.error(`Default CEF Source Path not found: '${cefSourcePath}'. Please ensure CEF is exported using 'cargo run -p export-cef-dir -- --force $HOME/.local/share/cef' or provide a valid path.`);
    Deno.exit(1);
  }
  
  // Set CEF_PATH for this script's scope
  Deno.env.set("CEF_PATH", cefSourcePath);
  console.log(`Using CEF_PATH: ${cefSourcePath}`);
  
  // Determine CEF Binary directory (might be Release subdir on Windows)
  let cefBinDir = cefSourcePath;
  const releasePath = path.join(cefSourcePath, "Release");
  try {
    const releaseInfo = await Deno.stat(releasePath);
    if (releaseInfo.isDirectory) {
      cefBinDir = releasePath;
      console.log(`Using CEF binaries from: ${cefBinDir}`);
    } else {
      console.log(`Using CEF binaries from: ${cefBinDir}`);
    }
  } catch {
    console.log(`Using CEF binaries from: ${cefBinDir}`);
  }
  
  // Calculate build output directory
  const buildOutputDir = path.join(scriptDir, cargoTargetDir, profile, "examples");
  console.log(`Build output directory set to: ${buildOutputDir}`);
  
  // Resolve the final output directory path
  const finalOutputFullPath = path.resolve(scriptDir, finalOutputDir);
  console.log(`Final output will be placed in: ${finalOutputFullPath}`);
  
  
  // --- 2. Build the Rust Example ---
  console.log(`Building example '${example}' with profile '${profile}'...`);
  
  const cargoBuild = new Deno.Command("cargo", {
    args: ["build", "--profile", profile, "--example", example],
    env: {
      "CEF_PATH": cefSourcePath,
      "PATH": `${Deno.env.get("PATH")}${path.DELIMITER}${cefBinDir}`
    },
    stdout: "inherit",
    stderr: "inherit"
  });
  
  const cargoOutput = await cargoBuild.output();
  if (!cargoOutput.success) {
    console.error("Cargo build failed!");
    Deno.exit(cargoOutput.code);
  }
  console.log("Build successful.");
  
  // Verify the executable was created
  const exeExtension = Deno.build.os === "windows" ? ".exe" : "";
  const exePath = path.join(buildOutputDir, `${example}${exeExtension}`);
  
  try {
    await Deno.stat(exePath);
  } catch {
    console.error(`Executable not found after build at ${exePath}. Build might have failed silently or output is elsewhere.`);
    Deno.exit(1);
  }
  
  
  // --- 3. Copy CEF Runtime Dependencies ---
  
  console.log(`Copying CEF runtime files to build output directory: ${buildOutputDir}`);
  
  // Files and Directories to Copy
  const filesToCopy = [
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
    "libEGL.dll",
    "libGLESv2.dll",
    "vk_swiftshader_icd.json"
  ];
  
  // Only include dxcompiler.dll if explicitly requested
  if (includeDxCompiler) {
    filesToCopy.push("dxcompiler.dll");
    filesToCopy.push("dxil.dll");
  }
  
  // Ensure build output directory exists
  try {
    await Deno.stat(buildOutputDir);
  } catch {
    await Deno.mkdir(buildOutputDir, { recursive: true });
  }
  
  // Copy Files
  for (const file of filesToCopy) {
    const sourceFile = path.join(cefBinDir, file);
    try {
      await Deno.stat(sourceFile);
      await Deno.copyFile(sourceFile, path.join(buildOutputDir, file));
    } catch {
      console.warn(`CEF source file not found: ${sourceFile}`);
    }
  }
  
  // Copy Locales - with optimization if enabled
  const localesDir = path.join(cefBinDir, "locales");
  const destLocalesDir = path.join(buildOutputDir, "locales");
  
  try {
    await Deno.stat(localesDir);
    
    // Remove destination if it exists
    try {
      await Deno.remove(destLocalesDir, { recursive: true });
    } catch {
      // Ignore if it doesn't exist
    }
    
    // Create the locales directory
    await Deno.mkdir(destLocalesDir, { recursive: true });
    
    if (minLocales) {
      // Only copy en-US.pak when minimizing
      const enUsPakPath = path.join(localesDir, "en-US.pak");
      try {
        await Deno.stat(enUsPakPath);
        await Deno.copyFile(enUsPakPath, path.join(destLocalesDir, "en-US.pak"));
      } catch {
        console.warn(`English locale file not found: ${enUsPakPath}`);
      }
    } else {
      // Copy all locales
      await fs.copy(localesDir, destLocalesDir, { overwrite: true });
    }
  } catch {
    console.warn(`CEF locales directory not found: ${localesDir}`);
  }
  
  // Copy Manifest if exists (specific to cefsimple example)
  if (example === "cefsimple") {
    const manifestSource = path.join(scriptDir, "cef", "examples", "cefsimple", "win", "cefsimple.exe.manifest");
    try {
      await Deno.stat(manifestSource);
      await Deno.copyFile(manifestSource, path.join(buildOutputDir, "cefsimple.exe.manifest"));
    } catch {
      console.warn(`Manifest file not found: ${manifestSource}`);
    }
  }
  
  console.log("Dependency copying complete.");
  
  
  // --- 4. Move Build Output to Final Location ---
  
  console.log(`Moving build output from ${buildOutputDir} to ${finalOutputFullPath}`);
  
  // Ensure final directory exists and is empty
  try {
    await Deno.stat(finalOutputFullPath);
    
    // Remove contents of directory
    for await (const entry of Deno.readDir(finalOutputFullPath)) {
      await Deno.remove(path.join(finalOutputFullPath, entry.name), { recursive: true });
    }
  } catch {
    // Create directory if it doesn't exist
    await Deno.mkdir(finalOutputFullPath, { recursive: true });
  }
  
  // Move the contents
  console.log("Moving files...");
  for await (const entry of Deno.readDir(buildOutputDir)) {
    const sourcePath = path.join(buildOutputDir, entry.name);
    const destPath = path.join(finalOutputFullPath, entry.name);
    
    // Skip PDB files if skipPdb is true
    if (skipPdb && entry.name.endsWith('.pdb')) {
      console.log(`Skipping debug symbols file: ${entry.name}`);
      continue;
    }
    
    // Use copy then delete instead of move for cross-device safety
    if (entry.isDirectory) {
      await fs.copy(sourcePath, destPath, { overwrite: true });
      await Deno.remove(sourcePath, { recursive: true });
    } else {
      await Deno.copyFile(sourcePath, destPath);
      await Deno.remove(sourcePath);
    }
  }
  
  // --- 5. Apply UPX Compression if available ---
  if (useUpx) {
    console.log("Checking for UPX...");
    let upxAvailable = false;
    
    try {
      const upxCheck = new Deno.Command(Deno.build.os === "windows" ? "where" : "which", {
        args: ["upx"],
        stdout: "piped",
        stderr: "piped",
      });
      
      const upxResult = await upxCheck.output();
      upxAvailable = upxResult.success;
    } catch {
      // Command failed, UPX not available
      upxAvailable = false;
    }
    
    if (upxAvailable) {
      console.log("UPX found, compressing executables...");

      // Optionally compress chrome_elf.dll which is usually safe to compress
      const libcefpath = path.join(finalOutputFullPath, "libcef.dll");
      try {
        await Deno.stat(libcefpath);

        const upxCommand = new Deno.Command("upx", {
          args: ["--best","--force", libcefpath],
          stdout: "inherit",
          stderr: "inherit",
        });

        const upxResult = await upxCommand.output();
        if (upxResult.success) {
          console.log("Successfully compressed libcef.dll with UPX");
        }
      } catch {
        // Ignore errors for optional compression
      }
      

      
      // Optionally compress chrome_elf.dll which is usually safe to compress
      const chromeElfPath = path.join(finalOutputFullPath, "chrome_elf.dll");
      try {
        await Deno.stat(chromeElfPath);
        
        const upxCommand = new Deno.Command("upx", {
          args: ["--best", chromeElfPath],
          stdout: "inherit",
          stderr: "inherit",
        });
        
        const upxResult = await upxCommand.output();
        if (upxResult.success) {
          console.log("Successfully compressed chrome_elf.dll with UPX");
        }
      } catch {
        // Ignore errors for optional compression
      }
    } else {
      console.warn("UPX not found in PATH. Skipping compression. Install UPX for smaller builds.");
    }
  }
  
  console.log(`Build and packaging complete. Output is in ${finalOutputFullPath}`);
}

// Run the main function
buildExample().catch(err => {
  console.error("Build failed with error:", err);
  Deno.exit(1);
});
