# `variables.tf` declares the settings of the Warped cloud deploy, which the Makefile passes from
# `.env` as `TF_VAR_*`.
#
# Interface:
#     project_id, region, zone, monthly_budget_usd, game_password (see `.env.example`);
#     has_game_servers, whether to run game servers or only the download page; games_file, the
#     list of games (`games.txt` unless the Makefile's GAMES_FILE says otherwise).
#
# Example:
#     TF_VAR_project_id=my-project terraform plan

variable "project_id" {
  type = string
}

variable "region" {
  type = string
}

variable "zone" {
  type = string
}

variable "monthly_budget_usd" {
  type = number
}

variable "has_game_servers" {
  type = bool
}

variable "games_file" {
  type = string
}

variable "game_password" {
  type      = string
  sensitive = true
  validation {
    condition     = length(var.game_password) > 0
    error_message = "game_password is empty; set GAME_PASSWORD in .env, or run make deploy-everything, which makes one."
  }
}
