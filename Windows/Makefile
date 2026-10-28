# Anemobit for Windows: build, test, and publish the executable and MSI installer with the dotnet CLI (run under Windows/).
#
#   make                 Same as make build
#   make build           Build the executable: a single framework-dependent Anemobit.exe (x64 and arm64)
#                        → src/Anemobit/bin/publish/win-<arch>/Anemobit.exe
#   make installer       Build the MSI installers (x64 and arm64) → ../Outputs/Anemobit-<version>-<arch>.msi
#   make debug           Debug build of the whole solution (app, Core, tests)
#   make test            Run all unit tests
#   make check           debug + test (must pass at least this after code changes)
#   make run             Run the app in Debug
#   make clean           Delete all build output (bin/, obj/, TestResults/, the MSIs make installer writes to ../Outputs) and the current user's
#                        Anemobit data (%APPDATA%\Anemobit, Credential Manager entries, Open at Login); quits a running Anemobit first
#   make help            Show this help
#
# One architecture only: make build ARCHS=x64, make installer ARCHS=arm64
#
# Requires GNU make (Windows build), the .NET 10 SDK, and PowerShell 7 (pwsh).
# Commands only use dotnet and pwsh, regardless of whether make runs them via cmd or sh.

ARCHS ?= x64 arm64

comma := ,

APP       := src/Anemobit
INSTALLER := installer/Anemobit.Installer
OUTPUTS   := ../Outputs
PWSH      := pwsh -NoProfile -NonInteractive -Command
# Version in Directory.Build.props (the MSI file names include it); read only when needed.
VERSION    = $(shell $(PWSH) "(Select-Xml -Path Directory.Build.props -XPath '//Version').Node.InnerText")
# Exactly the MSIs make installer writes for this version (other files in Outputs/ are kept).
MSIS       = $(foreach arch,$(ARCHS),$(OUTPUTS)/Anemobit-$(VERSION)-$(arch).msi)

.PHONY: all build installer debug test check run clean help $(addprefix build-,$(ARCHS)) $(addprefix installer-,$(ARCHS))

all: build

build: $(addprefix build-,$(ARCHS))

# Static pattern rules: plain pattern rules (build-%:) are never used for .PHONY targets, so make would do nothing.
$(addprefix build-,$(ARCHS)): build-%:
	dotnet publish $(APP) -p:PublishProfile=win-$*
	@$(PWSH) "Write-Output 'Anemobit.exe ($*) -> $(APP)/bin/publish/win-$*/Anemobit.exe'"

# The wixproj first publishes the executable for that architecture, then packages it as an MSI.
installer: $(addprefix installer-,$(ARCHS))

$(addprefix installer-,$(ARCHS)): installer-%:
	dotnet build $(INSTALLER) -c Release -p:Platform=$*

debug:
	dotnet build

test:
	dotnet test

check: debug
	dotnet test --no-build

run:
	dotnet run --project $(APP)

# dotnet clean doesn't remove obj/ or the publish folders, so delete them directly. From Outputs/, only the MSIs make installer produces for
# the current version and ARCHS are deleted (by exact name, never a wildcard), so other files there are kept.
# Also removes the current user's data, like the uninstaller's --uninstall-cleanup (Settings/UserDataCleanup.cs), so the system is left clean;
# this includes an installed Anemobit's settings, since all copies share them. A running Anemobit is quit first so it doesn't write the settings again
# or lock bin/. Recipes avoid '$' so they work the same whether make runs them via cmd or sh; "exit 0" because missing items aren't errors.
clean:
	$(PWSH) "Stop-Process -Name Anemobit -ErrorAction SilentlyContinue; exit 0"
	$(PWSH) "Get-ChildItem -Path src, tests, installer -Directory -Recurse -Include bin, obj | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue"
	$(PWSH) "Remove-Item -Recurse -Force -ErrorAction SilentlyContinue TestResults, .vs; exit 0"
	$(PWSH) "Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $(subst $() ,$(comma),$(strip $(MSIS))); exit 0"
	$(PWSH) "Remove-Item -Recurse -Force -ErrorAction SilentlyContinue (Join-Path ([Environment]::GetFolderPath('ApplicationData')) Anemobit); exit 0"
	$(PWSH) "cmdkey /delete:Anemobit:snmp/community | Out-Null; cmdkey /delete:Anemobit:snmp/authPassword | Out-Null; cmdkey /delete:Anemobit:snmp/privPassword | Out-Null; exit 0"
	$(PWSH) "Remove-ItemProperty -Name Anemobit -ErrorAction SilentlyContinue -Path HKCU:\Software\Microsoft\Windows\CurrentVersion\Run, HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run; exit 0"

help:
	$(PWSH) "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-Content -Encoding utf8 Makefile -TotalCount 18"
