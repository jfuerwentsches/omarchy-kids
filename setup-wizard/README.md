# setup-wizard

One-time first-boot setup on the child computer, hooking into Omarchy 4's deferred first-boot provisioning.

Asks for the child's name and initial age tier, sets branding (`omarchy ascii "<name>"` into `screensaver.txt`/`about.txt`), calls `omarchy-kids-set-tier <initial>`, creates the parent/admin account + `omarchy-kids-agent`, generates the SSH keypair.

Not yet implemented.
