# Infra

The cloud deploy on Google Cloud, in Terraform, so players can play over the internet (W-42 (server in the cloud)):
one small VM runs the online match (W-127 (online match)), every level protected by the game's password, and a public
bucket serves the download page with the app that plays all five levels (W-116 (download page)).

## Set up once

- Install Terraform (`brew tap hashicorp/tap && brew install hashicorp/tap/terraform`) and the Google Cloud CLI, then
  log in twice, the second time for Terraform: `gcloud auth login` and `gcloud auth application-default login`.
- The project needs billing turned on (its free VM still needs a billing account).
- `cp .env.example .env` and fill it in: the project's ID, region and zone, and the monthly budget. `.env` is
  git-ignored, and so is `.build/`, where Terraform keeps its working files (`TF_DATA_DIR`).

## Deploy in one click

Any game can use this: `GAME_DIR` names its folder (the match, `../demos/warped_match_rust`, by default), whose
Makefile must have:

- `build-server-bundle`: `.build/server_bundle.tar.gz`, unpacked on the VM in `/opt/warped/`, with a `services.txt`
  of one service a line, `<port> <program> <arguments>`. The VM runs each as `/opt/warped/<program> --port <port>
  <arguments> --game-password-file <the password>`, in its own writable folder, `/var/lib/warped-<port>`
  ([`startup.sh.tftpl`](startup.sh.tftpl)). The firewall opens UDP 47800 to 47899.
- `package-mac-app SERVER_ADDRESS=<ip>`: `.build/Warped.zip`, whose app joins the services at `<ip>`.
- `check-server SERVER_ADDRESS=<ip> GAME_PASSWORD_FILE=<file>`: a short check that they answer.

`GAME_DIR=../demos/hyperbolic_wormholes_rust` deploys the Lunar Surface demo alone instead, with the games of its
`app/games.txt`.

```bash
make deploy-everything
make check-game-servers
```

- `deploy-everything` goes from an empty project to a download page: it writes a random `GAME_PASSWORD` into `.env`
  if there is none, prepares the project, applies the Terraform, uploads the server bundle (built on this Mac for
  Linux, so the VM never clones the repository) and restarts the VM, then builds the app with the online match at the
  VM's IP and uploads it with the [download page](site/index.html) and its [install script](site/install.sh). It
  prints the page's address; share it, and the password, with the players.
- Each step is also its own target: `deploy-game-servers` after changing the server, `deploy-download-page` after
  changing the app or the page.
- `check-game-servers`, a minute after a deploy, joins the VM's match with a bot from this Mac for 5 s.
- After a level changes, `make deploy-everything` again ships it: the bundle and the app are rebuilt from the
  checkout.
- Downloads are uploaded with `Cache-Control: no-store`, so a new deploy shows at once.

## Only the download page

`HAS_GAME_SERVERS=false` in `.env` deploys only the download page: its bucket and the budget, with no VM. The app then
has no online match on its title screen; players join one by typing its address ("Join a match by its address"),
such as one run from a checkout with `make serve-match`. Set it back to `true` and run `make deploy-everything` again
to add the game servers.

## See what's running

Everything this deploy makes carries the label `app=warped`:

```bash
make list-gcp-resources
```

In the console (replace `<project>` with `GCP_PROJECT_ID`):

- VMs: `https://console.cloud.google.com/compute/instances?project=<project>`
- static IPs: `https://console.cloud.google.com/networking/addresses/list?project=<project>`
- buckets (the page, the server bundle, Terraform's state): `https://console.cloud.google.com/storage/browser?project=<project>`
- everything with the label: the console's search bar, `labels.app:warped`
- spending and the budget: Billing, then Reports and Budgets & alerts.

## Shut everything down

```bash
make destroy-infra YES_TO_DESTROY_INFRA=1
gcloud storage rm -r gs://<project>-tfstate
```

- `destroy-infra` deletes the VM, its IP, the firewall rules, both buckets and the budget; the second line deletes
  Terraform's state bucket, which `prepare-gcp-project` made. The APIs stay enabled, which costs nothing.
- If the project exists only for this, `gcloud projects delete <project>` removes all of it at once.
- Afterwards `make list-gcp-resources` lists nothing.

## Make it step by step

```bash
make prepare-gcp-project
make plan-infra
make apply-infra
make destroy-infra YES_TO_DESTROY_INFRA=1
```

- `prepare-gcp-project` enables the APIs and makes the bucket that holds Terraform's state
  (`<project>-tfstate`, versioned), so the deploy works from any computer; it is safe to rerun.
- `apply-infra` makes or updates, in [`main.tf`](main.tf):
  - an e2-micro VM (Debian 12, 10 GB standard disk) with a static IP on the Standard network tier;
  - a firewall that lets anyone reach the games' UDP ports, and SSH only through Google's IAP;
  - a public bucket for the download page and a private one for the server bundle, which only the VM reads;
  - a budget that emails the billing account's admins at half and all of `MONTHLY_BUDGET_USD`.
- On every boot the VM runs [`startup.sh.tftpl`](startup.sh.tftpl): it downloads the newest server bundle, writes
  the game's password from its metadata, and restarts one systemd service, `warped-game@<port>`, per game.
- `destroy-infra` deletes everything but the state bucket.

## Cost

The e2-micro VM, 30 GB of standard disk and 200 GB a month of Standard-tier traffic out are free in us-central1,
us-west1 and us-east1; the static IP costs a few dollars a month. Twenty players send about 4 GB an hour, so about
50 hours of full games a month are free.

## Debug

```bash
gcloud compute instances get-serial-port-output warped-game-server --zone us-central1-a
gcloud compute ssh warped-game-server --zone us-central1-a --tunnel-through-iap
```

The first shows the startup script's output; on the VM, `journalctl -u 'warped-game@*'` shows the game servers'.
