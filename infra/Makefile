# `Makefile` sets up and tears down the Warped cloud deploy on Google Cloud with Terraform, reading
# its settings from `.env` (copy `.env.example`).
#
# Interface:
#     make help: list the targets.
#     make prepare-gcp-project: enable the APIs and create the bucket for Terraform's state (safe to rerun).
#     make plan-infra: show what apply-infra would change.
#     make apply-infra: create or update the VM, its IP, the buckets, the firewall and the budget.
#     make destroy-infra YES_TO_DESTROY_INFRA=1: delete all of them (or INTERACTIVE=1 to be asked).
#     make create-game-password: write a random GAME_PASSWORD into .env if it has none.
#     make deploy-game-servers: build the server bundle, upload it and restart the VM, which runs it.
#     make deploy-download-page: build the app with the online games and upload it with the download page.
#     make deploy-everything: all of the above in order, from an empty project to a download page (one click).
#     make check-game-servers: join every online game from this Mac with a bot, to check that they answer.
#     make list-gcp-resources: list what this deploy made (everything labelled app=warped), and its state bucket.
#     make test: check the Terraform, plan it with and without game servers against a mock provider, and
#         render and check the VM's startup script.
#     make fmt: format the Terraform.
#
# Example:
#     cp .env.example .env && make deploy-everything
#
# Must know:
#     - HAS_GAME_SERVERS=false in .env deploys only the download page: no VM, and no online games in
#       the app (players join a server by its address).
#     - GAME_DIR is the game to deploy (the Rust demo by default): any game folder whose Makefile has
#       build-server-bundle and package-mac-app, and GAMES_FILE lists its games (port, map, name).
SHELL := /bin/bash
.SHELLFLAGS := -eo pipefail -c
-include .env
GCLOUD ?= gcloud
TERRAFORM ?= terraform
STATE_BUCKET := $(GCP_PROJECT_ID)-tfstate
APIS := compute.googleapis.com storage.googleapis.com iap.googleapis.com billingbudgets.googleapis.com \
  cloudbilling.googleapis.com cloudresourcemanager.googleapis.com iam.googleapis.com
TEST_DATA_DIR := .build/terraform-test
GAME_DIR ?= ../demos/hyperbolic_wormholes_rust
GAME_PROGRAM := $(GAME_DIR)/.build/release/hyperbolic-wormholes
GAMES_FILE ?= games.txt
HAS_GAME_SERVERS ?= true
WITH_GAME_SERVERS := $(filter true,$(HAS_GAME_SERVERS))
RESOURCE_LABEL := app=warped
SITE_BUILD := .build/site
CHECK_S := 5
SITE_HOST ?= https://storage.googleapis.com
export TF_DATA_DIR := .build/terraform
export TF_VAR_project_id := $(GCP_PROJECT_ID)
export TF_VAR_region := $(GCP_REGION)
export TF_VAR_zone := $(GCP_ZONE)
export TF_VAR_monthly_budget_usd := $(MONTHLY_BUDGET_USD)
export TF_VAR_game_password := $(GAME_PASSWORD)
export TF_VAR_games_file := $(abspath $(GAMES_FILE))
export TF_VAR_has_game_servers := $(HAS_GAME_SERVERS)

DESTROY_INFRA_WARNING := This deletes the game server VM, its IP, both buckets (the download page and the \
  server bundle), the firewall rules and the budget: players are cut off, and the IP changes when it is made \
  again, so the app must be downloaded again. To skip this question, pass YES_TO_DESTROY_INFRA=1.

.PHONY: help check-env prepare-gcp-project init-terraform plan-infra apply-infra destroy-infra create-game-password \
  deploy-game-servers deploy-download-page deploy-everything check-game-servers list-gcp-resources test fmt

help:
	@echo "Targets (settings in .env; copy .env.example):"
	@echo "  make prepare-gcp-project                    Enable the APIs and make Terraform's state bucket (safe to rerun)"
	@echo "  make plan-infra                             Show what apply-infra would change"
	@echo "  make apply-infra                            Create or update the VM, IP, buckets, firewall and budget"
	@echo "  make destroy-infra YES_TO_DESTROY_INFRA=1   Delete them all (or INTERACTIVE=1 to be asked)"
	@echo "  make create-game-password                   Write a random GAME_PASSWORD into .env if it has none"
	@echo "  make deploy-game-servers                    Build and upload the server bundle, then restart the VM"
	@echo "  make deploy-download-page                   Build the app with the online games and upload the page"
	@echo "  make deploy-everything                      All of the above, from an empty project (one click)"
	@echo "  make check-game-servers                     Join every online game with a bot from this Mac"
	@echo "  make list-gcp-resources                     List what this deploy made in Google Cloud"
	@echo "  make test                                   Check the Terraform and the VM's startup script"
	@echo "  make fmt                                    Format the Terraform"
	@echo ""
	@echo "Example: cp .env.example .env && make deploy-everything"

check-env:
	@test -f .env || { echo "No .env here: run cp .env.example .env and fill it in." >&2; exit 2; }
	@test -n "$(GCP_PROJECT_ID)" -a -n "$(GCP_REGION)" -a -n "$(GCP_ZONE)" -a -n "$(MONTHLY_BUDGET_USD)" || \
		{ echo ".env needs GCP_PROJECT_ID, GCP_REGION, GCP_ZONE and MONTHLY_BUDGET_USD." >&2; exit 2; }
	@test "$(HAS_GAME_SERVERS)" = true -o "$(HAS_GAME_SERVERS)" = false || \
		{ echo "HAS_GAME_SERVERS in .env must be true or false, not '$(HAS_GAME_SERVERS)'." >&2; exit 2; }

prepare-gcp-project: check-env
	@test "$$($(GCLOUD) billing projects describe $(GCP_PROJECT_ID) --format='value(billingEnabled)')" = True || \
		{ echo "Billing isn't enabled for $(GCP_PROJECT_ID); link a billing account in the console first." >&2; exit 1; }
	$(GCLOUD) services enable $(APIS) --project $(GCP_PROJECT_ID)
	$(GCLOUD) storage buckets describe gs://$(STATE_BUCKET) --project $(GCP_PROJECT_ID) > /dev/null 2>&1 || \
		$(GCLOUD) storage buckets create gs://$(STATE_BUCKET) --project $(GCP_PROJECT_ID) \
		--location $(GCP_REGION) --uniform-bucket-level-access
	$(GCLOUD) storage buckets update gs://$(STATE_BUCKET) --versioning

init-terraform: check-env
	$(TERRAFORM) init -input=false -reconfigure -backend-config=bucket=$(STATE_BUCKET)

plan-infra: init-terraform
	$(TERRAFORM) plan -input=false

apply-infra: init-terraform
	$(TERRAFORM) apply -input=false -auto-approve

destroy-infra: init-terraform
ifeq ($(YES_TO_DESTROY_INFRA),1)
	$(TERRAFORM) destroy -input=false -auto-approve
else ifeq ($(INTERACTIVE),1)
	@printf '%s\nType yes to confirm: ' "$(DESTROY_INFRA_WARNING)"; read answer; [ "$$answer" = yes ]
	$(TERRAFORM) destroy -input=false -auto-approve
else
	@printf '%s\nTo be asked instead, pass INTERACTIVE=1.\n' "$(DESTROY_INFRA_WARNING)" >&2; exit 2
endif

create-game-password: check-env
	@if [ -z "$(GAME_PASSWORD)" ]; then \
		password="$$(openssl rand -hex 8)"; \
		if grep -q '^GAME_PASSWORD=' .env; then \
			sed -i '' "s/^GAME_PASSWORD=.*/GAME_PASSWORD=$$password/" .env; \
		else \
			echo "GAME_PASSWORD=$$password" >> .env; \
		fi; \
		echo "Wrote a new game password into .env (GAME_PASSWORD)."; \
	fi

deploy-game-servers: init-terraform
	$(MAKE) -C $(GAME_DIR) build-server-bundle
	$(GCLOUD) storage cp $(GAME_DIR)/.build/server_bundle.tar.gz \
		gs://$$($(TERRAFORM) output -raw server_bucket)/server_bundle.tar.gz
	$(GCLOUD) compute instances reset warped-game-server --zone $(GCP_ZONE) --project $(GCP_PROJECT_ID)

deploy-download-page: init-terraform
	mkdir -p $(SITE_BUILD)
	$(if $(WITH_GAME_SERVERS),awk -v ip="$$($(TERRAFORM) output -raw server_ip)" '{ $$1 = ip ":" $$1; print }' \
		$(GAMES_FILE) > .build/online_games.txt)
	$(MAKE) -C $(GAME_DIR) package-mac-app $(if $(WITH_GAME_SERVERS),ONLINE_GAMES=$(CURDIR)/.build/online_games.txt)
	site_url="$(SITE_HOST)/$$($(TERRAFORM) output -raw site_bucket)"; \
		sed "s|@SITE_URL@|$$site_url|g" site/index.html > $(SITE_BUILD)/index.html; \
		sed "s|@SITE_URL@|$$site_url|g" site/install.sh > $(SITE_BUILD)/install.sh
	cp $(GAME_DIR)/.build/Warped.zip $(GAME_DIR)/app/app_icon.png $(SITE_BUILD)/
	$(GCLOUD) storage cp --cache-control=no-store $(SITE_BUILD)/* gs://$$($(TERRAFORM) output -raw site_bucket)/
	@echo "Download page: $$($(TERRAFORM) output -raw download_page_url)"

deploy-everything: check-env
	$(MAKE) create-game-password
	$(MAKE) prepare-gcp-project
	$(MAKE) apply-infra
	$(if $(WITH_GAME_SERVERS),$(MAKE) deploy-game-servers)
	$(MAKE) deploy-download-page
	@echo "Done. Share the download page above$(if $(WITH_GAME_SERVERS), and the game password (GAME_PASSWORD in .env))."
	@$(if $(WITH_GAME_SERVERS),echo "Give the VM a minute to start the games; then check them with: make check-game-servers")
	@echo "See what exists with: make list-gcp-resources; remove it with: make destroy-infra YES_TO_DESTROY_INFRA=1"

check-game-servers: init-terraform
	cd $(GAME_DIR) && cargo build --release
	install -m 600 /dev/null .build/game_password
	printf '%s\n' "$(GAME_PASSWORD)" > .build/game_password
	server_ip="$$($(TERRAFORM) output -raw server_ip)"; \
		while read -r port map name; do \
			echo "Checking $$name ($$server_ip:$$port, $$map)"; \
			$(GAME_PROGRAM) --join "$$server_ip:$$port" --map "$$map" \
				--game-password-file $(CURDIR)/.build/game_password --bot --exit-after-s $(CHECK_S) --label "check-$$port"; \
		done < $(GAMES_FILE)

list-gcp-resources: check-env
	@echo "Everything labelled $(RESOURCE_LABEL) in $(GCP_PROJECT_ID) (the budget has no labels: see Billing, Budgets):"
	$(GCLOUD) compute instances list --project $(GCP_PROJECT_ID) --filter="labels.$(RESOURCE_LABEL)"
	$(GCLOUD) compute addresses list --project $(GCP_PROJECT_ID) --filter="labels.$(RESOURCE_LABEL)"
	$(GCLOUD) compute firewall-rules list --project $(GCP_PROJECT_ID) --filter="name~^warped-"
	$(GCLOUD) storage buckets list --project $(GCP_PROJECT_ID) --format="table(name, location, labels)"

test:
	$(TERRAFORM) fmt -check
	TF_DATA_DIR=$(TEST_DATA_DIR) $(TERRAFORM) init -input=false -backend=false > /dev/null
	TF_DATA_DIR=$(TEST_DATA_DIR) $(TERRAFORM) validate
	TF_DATA_DIR=$(TEST_DATA_DIR) $(TERRAFORM) test
	mkdir -p .build/render
	echo 'templatefile("$(CURDIR)/startup.sh.tftpl", {bucket = "bucket", games = [{port = 47810, map = "lunar-surface"}]})' | \
		$(TERRAFORM) -chdir=.build/render console | sed -e '1d' -e '$$d' > .build/startup.sh
	bash -n .build/startup.sh
	bash -n site/install.sh
	@echo "Rendered startup script: .build/startup.sh"

fmt:
	$(TERRAFORM) fmt
