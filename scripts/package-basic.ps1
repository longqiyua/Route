param(
    [Parameter(Mandatory=$true)][string]$Binary,
    [Parameter(Mandatory=$true)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$source=(Resolve-Path -LiteralPath $Binary).Path
$output=[IO.Path]::GetFullPath($OutputDirectory)
$allowed=[IO.Path]::GetFullPath((Join-Path $repo 'Release/packages')).TrimEnd('\','/')+[IO.Path]::DirectorySeparatorChar
if(-not $output.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)){throw 'Package must be a new child of Release/packages'}
if(Test-Path -LiteralPath $output){throw 'Refusing to overwrite an existing package'}
if(Test-Path -LiteralPath ($output+'.zip')){throw 'Refusing to overwrite an existing ZIP'}
if(Test-Path -LiteralPath ($output+'.manifest.json')){throw 'Refusing to overwrite an existing manifest'}
$inputFile=Get-Item -LiteralPath $source
if(-not $inputFile.PSIsContainer -and $inputFile.Extension -eq '.exe'){}else{throw 'Expected a built Windows executable'}
New-Item -ItemType Directory -Path $output | Out-Null
Copy-Item -LiteralPath $source -Destination (Join-Path $output 'route-ai.exe')
foreach($name in @('LICENSE','THIRD_PARTY_NOTICES.md')){Copy-Item -LiteralPath (Join-Path $repo $name) -Destination (Join-Path $output $name)}
$names=@('LICENSE','THIRD_PARTY_NOTICES.md','route-ai.exe')
Add-Type -AssemblyName System.IO.Compression
$zipStream=[IO.File]::Open(($output+'.zip'),[IO.FileMode]::CreateNew)
try{
 $zip=[IO.Compression.ZipArchive]::new($zipStream,[IO.Compression.ZipArchiveMode]::Create,$true)
 try{
  foreach($name in $names){
   $entry=$zip.CreateEntry($name,[IO.Compression.CompressionLevel]::Optimal)
   $entry.LastWriteTime=[DateTimeOffset]::new(2000,1,1,0,0,0,[TimeSpan]::Zero)
   $dest=$entry.Open();$src=[IO.File]::OpenRead((Join-Path $output $name))
   try{$src.CopyTo($dest)}finally{$src.Dispose();$dest.Dispose()}
  }
 }finally{$zip.Dispose()}
}finally{$zipStream.Dispose()}
$files=@($names|ForEach-Object{$p=Join-Path $output $_;[ordered]@{path=$_;bytes=(Get-Item -LiteralPath $p).Length;sha256=(Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash}})
if(@(Get-ChildItem -LiteralPath $output -Force).Count -ne 3){throw 'Unexpected package files'}
$manifest=[ordered]@{schema='route.basic-package/1';public_release=$false;primary_executable='route-ai.exe';legacy_compatible_implementation='route';file_count=3;files=$files;exe_bytes=(Get-Item -LiteralPath (Join-Path $output 'route-ai.exe')).Length;zip_bytes=(Get-Item -LiteralPath ($output+'.zip')).Length;zip_sha256=(Get-FileHash -LiteralPath ($output+'.zip') -Algorithm SHA256).Hash;notices='Existing repository LICENSE and THIRD_PARTY_NOTICES retained; not a new public licensing certification'}
$manifest|ConvertTo-Json -Depth 8|Set-Content -LiteralPath ($output+'.manifest.json') -Encoding UTF8
$manifest|ConvertTo-Json -Depth 8
