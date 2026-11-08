# `variables.tf` declares the settings of the Warped cloud deploy, which the Makefile passes from
# `.env` as `TF_VAR_*`.
#
# Interface:
#     project_id, region, zone, monthly_budget_usd, game_password (see `.env.example`).
#
# Example:
#     TF_VAR_project_id=warped-zq607d terraform plan

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

variable "game_password" {
  type      = string
  sensitive = true
  validation {
    condition     = length(var.game_password) > 0
    error_message = "game_password is empty; set GAME_PASSWORD in .env, or run make deploy-everything, which makes one."
  }
}
