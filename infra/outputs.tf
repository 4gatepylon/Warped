# `outputs.tf` gives what the deploy steps and players need from the Warped cloud deploy.
#
# Interface:
#     server_ip: the game server's static IP; site_bucket, server_bucket: the buckets' names;
#     download_page_url: the public download page.
#
# Example:
#     terraform output -raw server_ip

output "server_ip" {
  value = google_compute_address.game_server.address
}

output "site_bucket" {
  value = google_storage_bucket.site.name
}

output "server_bucket" {
  value = google_storage_bucket.server.name
}

output "download_page_url" {
  value = "https://storage.googleapis.com/${google_storage_bucket.site.name}/index.html"
}
