# `game_servers.tftest.hcl` checks, against a mock Google provider (no account or network needed),
# what the deploy makes with game servers and with only the download page.
#
# Interface:
#     run "page_only", run "with_game_servers"; `make test` runs them with `terraform test`.
#
# Example:
#     terraform test

mock_provider "google" {}

variables {
  project_id         = "test-project"
  region             = "us-central1"
  zone               = "us-central1-a"
  monthly_budget_usd = 30
  game_password      = "test password"
}

run "page_only" {
  command = plan
  variables {
    has_game_servers = false
  }
  assert {
    condition     = length(google_compute_instance.game_server) == 0 && length(google_compute_address.game_server) == 0
    error_message = "Without game servers there is no VM and no IP."
  }
  assert {
    condition     = length(google_storage_bucket.server) == 0 && length(google_compute_firewall.games) == 0
    error_message = "Without game servers there is no server bucket and no firewall."
  }
  assert {
    condition     = google_storage_bucket.site.name == "test-project-site"
    error_message = "The download page's bucket is made either way."
  }
}

run "with_game_servers" {
  command = plan
  variables {
    has_game_servers = true
  }
  assert {
    condition     = length(google_compute_instance.game_server) == 1 && length(google_compute_firewall.games) == 1
    error_message = "With game servers there is one VM and its firewall."
  }
  assert {
    condition     = anytrue([for rule in google_compute_firewall.games[0].allow : contains(rule.ports, "47800-47899")])
    error_message = "The firewall opens Warped's UDP port range, whatever the bundle serves on."
  }
  assert {
    condition     = google_storage_bucket.site.name == "test-project-site"
    error_message = "The download page's bucket is made either way."
  }
}
