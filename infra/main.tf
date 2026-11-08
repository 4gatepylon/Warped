# `main.tf` is the Warped cloud deploy on Google Cloud: one small VM that runs the services of the
# server bundle's `services.txt` (the online match, W-127), a public bucket for the download page, a
# private bucket for the server bundle, and a monthly budget.
#
# Interface:
#     Variables in `variables.tf`, outputs in `outputs.tf`; the Makefile runs it.
#
# Example:
#     make apply-infra
#
# Must know:
#     - With has_game_servers false, only the download page's bucket and the budget exist: no VM, IP,
#       firewall or server bucket.
#     - The VM's startup script (`startup.sh.tftpl`) runs on every boot and fetches the newest
#       bundle, so resetting the VM deploys a new server.
#     - The firewall opens all of Warped's UDP ports (local.game_port_range), so a bundle may
#       serve on any of them.
#     - The e2-micro VM, its 30 GB of standard disk and 200 GB a month of Standard-tier traffic are
#       free in us-central1, us-west1 and us-east1; the static IP is not.

terraform {
  required_version = ">= 1.5"
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 7.0"
    }
  }
  backend "gcs" {
    prefix = "warped"
  }
}

provider "google" {
  project               = var.project_id
  region                = var.region
  zone                  = var.zone
  billing_project       = var.project_id
  user_project_override = true
  default_labels = {
    app = "warped"
  }
}

locals {
  game_port_range   = "47800-47899"
  game_server_tag   = "warped-game-server"
  game_server_count = var.has_game_servers ? 1 : 0
}

data "google_project" "this" {}

resource "google_compute_network" "warped" {
  count                   = local.game_server_count
  name                    = "warped"
  auto_create_subnetworks = true
}

resource "google_compute_firewall" "games" {
  count         = local.game_server_count
  name          = "warped-games"
  network       = google_compute_network.warped[0].id
  source_ranges = ["0.0.0.0/0"]
  target_tags   = [local.game_server_tag]
  allow {
    protocol = "udp"
    ports    = [local.game_port_range]
  }
}

resource "google_compute_firewall" "ssh_through_iap" {
  count         = local.game_server_count
  name          = "warped-ssh-through-iap"
  network       = google_compute_network.warped[0].id
  source_ranges = ["35.235.240.0/20"]
  target_tags   = [local.game_server_tag]
  allow {
    protocol = "tcp"
    ports    = ["22"]
  }
}

resource "google_compute_address" "game_server" {
  count        = local.game_server_count
  name         = "warped-game-server"
  region       = var.region
  network_tier = "STANDARD"
}

resource "google_storage_bucket" "site" {
  name                        = "${var.project_id}-site"
  location                    = var.region
  uniform_bucket_level_access = true
  force_destroy               = true
}

resource "google_storage_bucket_iam_member" "site_is_public" {
  bucket = google_storage_bucket.site.name
  role   = "roles/storage.objectViewer"
  member = "allUsers"
}

resource "google_storage_bucket" "server" {
  count                       = local.game_server_count
  name                        = "${var.project_id}-server"
  location                    = var.region
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"
  force_destroy               = true
}

resource "google_service_account" "game_server" {
  count        = local.game_server_count
  account_id   = "warped-game-server"
  display_name = "Warped game server"
}

resource "google_storage_bucket_iam_member" "game_server_reads_bundle" {
  count  = local.game_server_count
  bucket = google_storage_bucket.server[0].name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.game_server[0].email}"
}

resource "google_compute_instance" "game_server" {
  count                     = local.game_server_count
  name                      = "warped-game-server"
  machine_type              = "e2-micro"
  zone                      = var.zone
  tags                      = [local.game_server_tag]
  allow_stopping_for_update = true
  boot_disk {
    initialize_params {
      image = "debian-cloud/debian-12"
      size  = 10
      type  = "pd-standard"
    }
  }
  network_interface {
    network = google_compute_network.warped[0].id
    access_config {
      nat_ip       = google_compute_address.game_server[0].address
      network_tier = "STANDARD"
    }
  }
  service_account {
    email  = google_service_account.game_server[0].email
    scopes = ["cloud-platform"]
  }
  metadata = {
    game-password = var.game_password
    startup-script = templatefile("${path.module}/startup.sh.tftpl", {
      bucket = google_storage_bucket.server[0].name
    })
  }
}

resource "google_billing_budget" "monthly" {
  billing_account = data.google_project.this.billing_account
  display_name    = "Warped monthly budget"
  budget_filter {
    projects = ["projects/${data.google_project.this.number}"]
  }
  amount {
    specified_amount {
      currency_code = "USD"
      units         = tostring(var.monthly_budget_usd)
    }
  }
  threshold_rules {
    threshold_percent = 0.5
  }
  threshold_rules {
    threshold_percent = 1.0
  }
}
