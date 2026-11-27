"""Apply OSX-Hyper-V's AMD patches to a prebuilt OpenCore config.plist.

Reproduces OCE-Build scripts/amd.py (commit b1f7eea4) with the standard library
only and pinned inputs, instead of running its unpinned bootstrap as admin:

  1. Kernel -> Emulate -> DummyPowerManagement = true
  2. every patch in AMD_Vanilla patches.plist (pinned commit), appended to
     Kernel -> Patch, with the VM's core count written into the
     cpuid_cores_per_package patches exactly as amd.py's byte replacements do
  3. the Hyper-V "flagers - kill invalid wrmsr" patch from the OSX-Hyper-V README

prepare.ps1 runs it; by hand:
    python -I apply-amd.py <config.plist> <AMD_Vanilla patches.plist> <cores>

<cores> must equal the VM's processor count (create-vm.ps1 -Cores).
"""
import plistlib
import shutil
import sys
from pathlib import Path

config_path = Path(sys.argv[1])
vanilla_path = Path(sys.argv[2])
cores = int(sys.argv[3])
if not 1 <= cores <= 255:
    sys.exit("core count must be 1..255")

backup = config_path.with_suffix(".plist.orig")
if not backup.exists():
    shutil.copy2(config_path, backup)
config = plistlib.loads(backup.read_bytes())  # always patch the pristine copy
vanilla = plistlib.loads(vanilla_path.read_bytes())

kernel = config.setdefault("Kernel", {})
kernel.setdefault("Emulate", {})["DummyPowerManagement"] = True

# amd.py's four textual replacements, as byte prefixes: the first byte stays,
# the second (zero) becomes the core count.
prefixes = [
    bytes.fromhex("B80000000000"),
    bytes.fromhex("BA0000000000"),
    bytes.fromhex("BA0000000090"),
    bytes.fromhex("BA00000000"),
]
patched_cores = 0
for patch in vanilla["Kernel"]["Patch"]:
    rep = patch.get("Replace", b"")
    for prefix in prefixes:
        if rep == prefix:
            patch["Replace"] = prefix[:1] + bytes([cores]) + prefix[2:]
            patched_cores += 1
            break

existing = kernel.setdefault("Patch", [])
have = {p.get("Comment") for p in existing}
added = [p for p in vanilla["Kernel"]["Patch"] if p.get("Comment") not in have]
existing.extend(added)

wrmsr = {
    "Arch": "x86_64",
    "Base": "_cpu_syscall_init",
    "Comment": "flagers - kill invalid wrmsr | 10.13+",
    "Count": 3,
    "Enabled": True,
    "Find": bytes.fromhex("0F30"),
    "Identifier": "kernel",
    "Limit": 0,
    "Mask": b"",
    "MaxKernel": "",
    "MinKernel": "17.0.0",
    "Replace": bytes.fromhex("9090"),
    "ReplaceMask": b"",
    "Skip": 0,
}
if wrmsr["Comment"] not in {p.get("Comment") for p in existing}:
    existing.append(wrmsr)

config_path.write_bytes(plistlib.dumps(config, fmt=plistlib.FMT_XML, sort_keys=False))

print(f"DummyPowerManagement: on")
print(f"AMD_Vanilla patches added: {len(added)} (core count {cores} written into {patched_cores})")
print(f"wrmsr patch: present")
print(f"Kernel->Patch now holds {len(existing)} entries; original kept at {backup.name}")
for p in vanilla["Kernel"]["Patch"]:
    if "cores_per_package" in p.get("Comment", ""):
        print(f"  {p['Comment']}: Replace={p['Replace'].hex().upper()}")
