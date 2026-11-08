# `Makefile` sets up and tears down the Warped cloud deploy on Google Cloud with Terraform, reading
# its settings from `.env` (copy `.env.example`).
#
# Interface:
#     make help: list the targets.
#     make prepare-gcp-project: enable the APIs and create the bucket for Terraform's state (safe to rerun).
#     make plan-infra: show what apply-infra would change.
#     make apply-infra: create or update the VM, its IP, the buckets, the firewall and the budget.
#     make destroy-infra YES_TO_DESTROY_INFRA=1: delete all of them (or INTERACTIVE=1 to be asked).
#     make test: check the Terraform and render and check the VM's startup script.
#     make fmt: format the Terraform.
#
# Example:
#     cp .env.example .env && make prepare-gcp-project apply-infra
SHELL := /bin/bash
.SHELLFLAGS := -eo pipefail -c
-include .env
GCLOUD ?= gcloud
TERRAFORM ?= terraform
STATE_BUCKET := $(GCP_PROJECT_ID)-tfstate
APIS := compute.googleapis.com storage.googleapis.com iap.googleapis.com billingbudgets.googleapis.com \
  cloudbilling.googleapis.com cloudresourcemanager.googleapis.com
TEST_DATA_DIR := .build/terraform-test
export TF_DATA_DIR := .build/terraform
export TF_VAR_project_id := $(GCP_PROJECT_ID)
export TF_VAR_region := $(GCP_REGION)
export TF_VAR_zone := $(GCP_ZONE)
export TF_VAR_monthly_budget_usd := $(MONTHLY_BUDGET_USD)
export TF_VAR_game_password := $(GAME_PASSWORD)

DESTROY_INFRA_WARNING := This deletes the game server VM, its IP, both buckets (the download page and the \
  server bundle), the firewall rules and the budget: players are cut off, and the IP changes when it is made \
  again, so the app must be downloaded again. To skip this question, pass YES_TO_DESTROY_INFRA=1.

.PHONY: help check-env prepare-gcp-project init-terraform plan-infra apply-infra destroy-infra test fmt

help:
	@echo "Targets (settings in .env; copy .env.example):"
	@echo "  make prepare-gcp-project                    Enable the APIs and make Terraform's state bucket (safe to rerun)"
	@echo "  make plan-infra                             Show what apply-infra would change"
	@echo "  make apply-infra                            Create or update the VM, IP, buckets, firewall and budget"
	@echo "  make destroy-infra YES_TO_DESTROY_INFRA=1   Delete them all (or INTERACTIVE=1 to be asked)"
	@echo "  make test                                   Check the Terraform and the VM's startup script"
	@echo "  make fmt                                    Format the Terraform"
	@echo ""
	@echo "Example: cp .env.example .env && make prepare-gcp-project apply-infra"

check-env:
	@test -f .env || { echo "No .env here: run cp .env.example .env and fill it in." >&2; exit 2; }
	@test -n "$(GCP_PROJECT_ID)" -a -n "$(GCP_REGION)" -a -n "$(GCP_ZONE)" -a -n "$(MONTHLY_BUDGET_USD)" || \
		{ echo ".env needs GCP_PROJECT_ID, GCP_REGION, GCP_ZONE and MONTHLY_BUDGET_USD." >&2; exit 2; }

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

test:
	$(TERRAFORM) fmt -check
	TF_DATA_DIR=$(TEST_DATA_DIR) $(TERRAFORM) init -input=false -backend=false > /dev/null
	TF_DATA_DIR=$(TEST_DATA_DIR) $(TERRAFORM) validate
	mkdir -p .build/render
	echo 'templatefile("$(CURDIR)/startup.sh.tftpl", {bucket = "bucket", games = [{port = 47810, map = "lunar-surface"}]})' | \
		$(TERRAFORM) -chdir=.build/render console | sed -e '1d' -e '$$d' > .build/startup.sh
	bash -n .build/startup.sh
	@echo "Rendered startup script: .build/startup.sh"

fmt:
	$(TERRAFORM) fmt
