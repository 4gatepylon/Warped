"""`capture_flows.py` plays the prototype's flows in Chrome, checks each screen and shoots it.

It starts its own server on a new world, so a run never depends on what someone did by hand, and it
fails at the first screen that doesn't show what it should. Each flow is a list of steps a person
would take; `VERIFY.md` beside `HANDOFF.md` points at the screenshots of each.

Interface:
    --port: The port of the server it starts, 1024 to 65535 (default: a free one).
    --flow: Only this flow; repeat it for several (default: every flow).
    --json: Print the result as one JSON object.
    Writes `.artifacts/capture_flows/<run_id>/`: `command.txt`, the server's `world.sqlite` and
    `server.log`, and `<flow>/<step>.png` per screen.

Example:
    $ uv run python scripts/capture_flows.py --flow sign_up

Must know:
    Dependencies: Google Chrome, which Playwright finds as its `chrome` channel.
"""

import json
import shlex
import socket
import subprocess
import sys
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from urllib.error import URLError
from urllib.request import urlopen

import click
from playwright.sync_api import Browser, BrowserContext, Page, expect, sync_playwright

_COMPONENT_FOLDER: Path = Path(__file__).resolve().parents[1]
_LOCAL_HOST: str = "127.0.0.1"
_MIN_PORT: int = 1024
_MAX_PORT: int = 65535
_SERVER_START_S: float = 30.0
_SERVER_POLL_S: float = 0.2
_VIEWPORT: dict[str, int] = {"width": 1280, "height": 960}
_SCREEN_TIMEOUT_MS: int = 10_000
_SUCCEEDING_CARD: str = "4242 4242 4242 4242"
_DECLINED_CARD: str = "4000 0000 0000 0002"


@dataclass
class _Run:
    """`_Run` is one run of the flows: its server, its browser and the screenshots taken so far.

    Example:
        >>> run.shoot(page, "home")
    """

    base_url: str
    browser: Browser
    flow_folder: Path
    screenshot_paths: list[str] = field(default_factory=list)

    def openComputer(self, region: str = "US") -> tuple[BrowserContext, Page]:
        """`openComputer` opens a new browser, which is one more computer, connecting from `region`.

        Example:
            >>> context, page = run.openComputer("FR")
        """
        context: BrowserContext = self.browser.new_context(
            viewport=_VIEWPORT, base_url=self.base_url
        )
        context.add_init_script(f"sessionStorage.setItem('warped_region', '{region}');")
        context.set_default_timeout(_SCREEN_TIMEOUT_MS)
        return context, context.new_page()

    def shoot(self, page: Page, step_name: str, is_full_page: bool = True) -> None:
        """`shoot` saves a screenshot of `page` as the flow's next step, all of it or what shows.

        Example:
            >>> run.shoot(page, "fake_google")
        """
        path: Path = self.flow_folder / f"{len(self.screenshot_paths) + 1:02d}-{step_name}.png"
        page.screenshot(path=path, full_page=is_full_page)
        self.screenshot_paths.append(str(path.relative_to(_COMPONENT_FOLDER)))


def _sign_up_with_google(run: _Run, page: Page, person: str, display_name: str) -> None:
    """`_sign_up_with_google` takes a new person through Google's page and the first-time form.

    Example:
        >>> _sign_up_with_google(run, page, "Ada Lovelace", "perihelion")
    """
    page.goto("/game/")
    page.get_by_role("button", name="SIGN IN WITH GOOGLE").click()
    page.get_by_role("button", name=person).click()
    expect(page.locator("#signup_who")).to_contain_text("Signed in with Google as")
    page.locator("#signup_name").fill(display_name)
    expect(page.locator("#signup_name_check")).to_have_text("free")
    page.locator("#signup_terms").check()
    page.get_by_role("button", name="CREATE ACCOUNT").click()
    expect(page.locator("#w-home")).to_contain_text(display_name)


def _play_sign_up(run: _Run) -> None:
    """`_play_sign_up` signs a new player up, showing the fake Google and each sign-up rule it hits.

    Example:
        >>> _play_sign_up(run)
    """
    _, page = run.openComputer()
    page.goto("/game/")
    run.shoot(page, "sign_in_window")
    page.get_by_role("button", name="SIGN IN WITH GOOGLE").click()
    expect(page.get_by_text("Choose an account")).to_be_visible()
    run.shoot(page, "fake_google")
    page.get_by_role("button", name="Ada Lovelace").click()
    expect(page.locator("#signup_who")).to_contain_text("ada")
    page.locator("#signup_name").fill("warped_admin")
    expect(page.locator("#signup_name_check")).to_contain_text("staff")
    page.locator("#signup_birth").fill("2015-01-01")
    page.get_by_role("button", name="CREATE ACCOUNT").click()
    expect(page.locator("#signup_error")).not_to_be_empty()
    run.shoot(page, "first_time_form_refused")
    page.locator("#signup_birth").fill("2000-01-01")
    page.locator("#signup_name").fill("perihelion")
    expect(page.locator("#signup_name_check")).to_have_text("free")
    page.locator("#signup_icons [data-icon='ring']").click()
    page.locator("#signup_terms").check()
    page.get_by_role("button", name="CREATE ACCOUNT").click()
    expect(page.locator("#home_access")).to_contain_text("Free online play left: 1 finished match")
    run.shoot(page, "home_signed_in")


def _play_purchase(run: _Run) -> None:
    """`_play_purchase` uses up the free allowance, is declined once, pays, and is refunded.

    Example:
        >>> _play_purchase(run)
    """
    context, page = run.openComputer()
    _sign_up_with_google(run, page, "Grace Hopper", "geodesic")
    page.locator("[data-account-act='play_match']").click()
    expect(page.locator("#home_access")).to_contain_text("used up")
    page.get_by_role("button", name="PLAY", exact=True).click()
    expect(page.locator("#w-purchase")).to_be_visible()
    run.shoot(page, "purchase_window")
    with context.expect_page() as stripe_page_info:
        page.get_by_role("button", name="PAY IN THE BROWSER").click()
    stripe_page: Page = stripe_page_info.value
    expect(stripe_page.locator("#checkout_amount")).to_have_text("$13.37")
    stripe_page.locator("#card_number").fill(_DECLINED_CARD)
    stripe_page.locator("#pay").click()
    expect(stripe_page.locator("#checkout_error")).to_contain_text("declined")
    run.shoot(stripe_page, "fake_stripe_declined")
    stripe_page.locator("#card_number").fill(_SUCCEEDING_CARD)
    stripe_page.locator("#pay").click()
    expect(stripe_page.locator("#w-paid")).to_be_visible()
    run.shoot(stripe_page, "website_paid")
    expect(page.locator("#w-home")).to_be_visible()
    expect(page.locator("#home_access")).to_have_text("Online play: bought.")
    run.shoot(page, "home_bought")
    page.locator("#sim_redeliver").click()
    page.locator("[data-account-act='refund']").click()
    expect(page.locator("#home_access")).to_contain_text("used up")
    run.shoot(page, "home_after_refund")
    page.goto("/system/")
    expect(page.locator("#tables")).to_contain_text("webhook_event")
    page.locator("#diagram").scroll_into_view_if_needed()
    run.shoot(page, "system_view", is_full_page=False)


def _play_sessions(run: _Run) -> None:
    """`_play_sessions` signs one account in on a second computer, which signs the first out.

    Example:
        >>> _play_sessions(run)
    """
    _, first_page = run.openComputer()
    _sign_up_with_google(run, first_page, "Emmy Noether", "noether")
    _, second_page = run.openComputer()
    second_page.goto("/game/")
    second_page.get_by_role("button", name="SIGN IN WITH GOOGLE").click()
    second_page.get_by_role("button", name="Emmy Noether").click()
    expect(second_page.locator("#home_access")).to_contain_text("Free online play left")
    expect(first_page.locator("#signin_reason")).to_contain_text("another computer")
    run.shoot(first_page, "first_computer_signed_out")
    second_page.goto("/game/#settings")
    second_page.get_by_role("button", name="ACCOUNT").click()
    second_page.locator("#settings_name").fill("noether2")
    second_page.get_by_role("button", name="SAVE NAME").click()
    second_page.locator("#settings_icons [data-icon='diamond']").click()
    expect(second_page.locator("#settings_name_wait")).to_contain_text("next change in 30 days")
    second_page.locator("#settings_name").fill("noether3")
    second_page.get_by_role("button", name="SAVE NAME").click()
    expect(second_page.locator("#settings_error")).to_contain_text("10 December 2026")
    run.shoot(second_page, "settings_name_too_soon")
    second_page.get_by_role("button", name="DELETE ACCOUNT").click()
    second_page.get_by_role("button", name="DELETE FOREVER").click()
    expect(second_page.locator("#signin_reason")).to_contain_text("deleted")
    run.shoot(second_page, "deleted")


def _play_eu(run: _Run) -> None:
    """`_play_eu` tries to sign up from France, which the game refuses.

    Example:
        >>> _play_eu(run)
    """
    _, page = run.openComputer("FR")
    page.goto("/game/")
    page.get_by_role("button", name="SIGN IN WITH GOOGLE").click()
    page.get_by_role("button", name="Sofia Kovalevskaya").click()
    expect(page.locator("#signup_who")).to_contain_text("sofia")
    page.locator("#signup_name").fill("kovalevskaya")
    page.locator("#signup_terms").check()
    page.get_by_role("button", name="CREATE ACCOUNT").click()
    expect(page.locator("#signup_error")).to_contain_text("European Union")
    run.shoot(page, "eu_refused")


_FLOWS: dict[str, Callable[[_Run], None]] = {
    "sign_up": _play_sign_up,
    "purchase": _play_purchase,
    "sessions": _play_sessions,
    "eu": _play_eu,
}


def _pick_free_port() -> int:
    """`_pick_free_port` returns a port nobody on this computer listens on now.

    Example:
        >>> port = _pick_free_port()
    """
    with socket.socket() as probe:
        probe.bind((_LOCAL_HOST, 0))
        return probe.getsockname()[1]


def _start_server(port: int, run_folder: Path) -> subprocess.Popen:
    """`_start_server` starts the prototype on a new world in `run_folder`, once it answers.

    Raises:
        click.ClickException: The server didn't answer within 30 seconds.

    Example:
        >>> server = _start_server(8011, run_folder)
    """
    log_file = (run_folder / "server.log").open("w")
    server: subprocess.Popen = subprocess.Popen(
        [
            *("uv", "run", "system-prototype", "--port", str(port)),
            *("--database-file", str(run_folder / "world.sqlite")),
        ],
        cwd=_COMPONENT_FOLDER,
        stdout=log_file,
        stderr=subprocess.STDOUT,
    )
    deadline_s: float = time.monotonic() + _SERVER_START_S
    while time.monotonic() < deadline_s:
        try:
            urlopen(f"http://{_LOCAL_HOST}:{port}/api/status").close()
            return server
        except URLError:
            time.sleep(_SERVER_POLL_S)
    server.terminate()
    raise click.ClickException(f"The server didn't answer on port {port}; see {run_folder}.")


@click.command(epilog="Example: uv run python scripts/capture_flows.py --flow sign_up")
@click.option(
    "--port", type=click.IntRange(_MIN_PORT, _MAX_PORT), help="Server port (default: free)."
)
@click.option(
    "--flow",
    "flow_names",
    type=click.Choice(list(_FLOWS)),
    multiple=True,
    help="Only this flow; repeat for several (default: every flow).",
)
@click.option("--json", "is_json", is_flag=True, help="Print the result as one JSON object.")
def main(port: int | None, flow_names: tuple[str, ...], is_json: bool) -> None:
    """`main` plays the chosen flows and prints the screenshots each produced."""
    run_id: str = datetime.now(UTC).strftime("%Y%m%d-%H%M%S")
    run_folder: Path = _COMPONENT_FOLDER / ".artifacts" / "capture_flows" / run_id
    run_folder.mkdir(parents=True)
    commit: str = subprocess.run(
        ["git", "describe", "--always", "--dirty"], capture_output=True, text=True, check=True
    ).stdout.strip()
    (run_folder / "command.txt").write_text(f"{shlex.join(sys.argv)}\ncommit {commit}\n")
    server_port: int = _pick_free_port() if port is None else port
    server: subprocess.Popen = _start_server(server_port, run_folder)
    screenshots_by_flow: dict[str, list[str]] = {}
    try:
        with sync_playwright() as playwright:
            browser: Browser = playwright.chromium.launch(channel="chrome")
            for flow_name in flow_names if len(flow_names) > 0 else tuple(_FLOWS):
                click.echo(f"playing {flow_name}", err=True)
                flow_folder: Path = run_folder / flow_name
                flow_folder.mkdir()
                run: _Run = _Run(f"http://{_LOCAL_HOST}:{server_port}", browser, flow_folder)
                _FLOWS[flow_name](run)
                screenshots_by_flow[flow_name] = run.screenshot_paths
            browser.close()
    finally:
        server.terminate()
    if is_json:
        click.echo(json.dumps({"run_id": run_id, "screenshots": screenshots_by_flow}))
        return
    for flow_name, paths in screenshots_by_flow.items():
        click.echo(f"{flow_name}: {len(paths)} screens")
        for path in paths:
            click.echo(f"  {path}")
    click.echo(f"run {run_id}: {run_folder.relative_to(_COMPONENT_FOLDER)}")


if __name__ == "__main__":
    main()
