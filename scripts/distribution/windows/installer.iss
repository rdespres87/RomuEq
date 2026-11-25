; ==============================================================================
; Equaform-8 DAW Plugins — Inno Setup 6 Installer Script
; Targets: 64-bit Windows 10 & 11 (x64)
; ==============================================================================

#define MyAppName "Equaform-8 DAW Plugins"
#ifndef MyAppVersion
  #define MyAppVersion "1.1.1"
#endif
#define MyAppPublisher "Equaform Audio"
#define MyAppURL "https://github.com/romueq/equaform"
#define MyAppId "{{D1A39C6B-70A5-42FE-9D72-B129D5B463AE}}"

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\Equaform
DefaultGroupName=Equaform Audio
DisableDirPage=yes
DisableProgramGroupPage=yes
OutputDir=..\..\..\target\release\dist
OutputBaseFilename=Equaform-8-Plugins-Windows-v{#MyAppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
CloseApplicationsFilter=*.exe
PrivilegesRequired=admin
UninstallDisplayIcon={autopf}\Equaform\unins000.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Types]
Name: "full"; Description: "Full Installation (VST3, CLAP, and Presets)"
Name: "custom"; Description: "Custom Installation"; Flags: iscustom

[Components]
Name: "vst3"; Description: "VST3 Plugin (Steinberg Cubase, Ableton Live, Studio One, Reaper, FL Studio)"; Types: full custom
Name: "clap"; Description: "CLAP Plugin (Bitwig Studio, Cockos Reaper, FL Studio)"; Types: full custom
Name: "presets"; Description: "15 Factory Presets (Mastering, Vocals, Bass Boost, EDM, etc.)"; Types: full custom

[Files]
; VST3 Bundle directory structure (Steinberg VST3 specification)
Source: "..\..\..\target\release\bundle\Equaform-8.vst3\*"; DestDir: "{commoncf64}\VST3\Equaform-8.vst3"; Flags: ignoreversion recursesubdirs createallsubdirs; Components: vst3

; CLAP Plugin binary (standard CLAP specification, also required by VST3 wrapper)
Source: "..\..\..\target\release\bundle\Equaform-8.clap"; DestDir: "{commoncf64}\CLAP"; Flags: ignoreversion; Components: clap or vst3

; Factory Presets deployed to shared ProgramData and user AppData
Source: "..\..\..\target\release\staging\presets\*.req"; DestDir: "{commonappdata}\Equaform\presets"; Flags: ignoreversion; Components: presets
Source: "..\..\..\target\release\staging\presets\*.req"; DestDir: "{userappdata}\Equaform\presets"; Flags: ignoreversion uninsneveruninstall; Components: presets

; Documentation & Testing Guide
Source: "README_WINDOWS.txt"; DestDir: "{autopf}\Equaform"; Flags: ignoreversion
Source: "TEST_GUIDE_WINDOWS.md"; DestDir: "{autopf}\Equaform"; Flags: ignoreversion

[Dirs]
Name: "{commoncf64}\VST3"
Name: "{commoncf64}\CLAP"
Name: "{commonappdata}\Equaform\presets"
Name: "{userappdata}\Equaform\presets"

[UninstallDelete]
; Clean up installed bundles while leaving custom user presets intact
Type: filesandordirs; Name: "{commoncf64}\VST3\Equaform-8.vst3"
Type: files; Name: "{commoncf64}\CLAP\Equaform-8.clap"
Type: filesandordirs; Name: "{commonappdata}\Equaform"
Type: filesandordirs; Name: "{autopf}\Equaform"
