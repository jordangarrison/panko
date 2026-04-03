defmodule PankoWeb.SessionsLiveTest do
  use PankoWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  setup %{conn: conn} do
    user = register_user()
    conn = log_in_user(conn, user)
    %{conn: conn, user: user}
  end

  test "renders empty state when no sessions", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/")
    assert render(view) =~ "No sessions found"
  end

  test "renders sessions list", %{conn: conn} do
    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, _session} = Panko.Sessions.import_from_file(path)

    {:ok, view, _html} = live(conn, ~p"/")

    # Expand the project accordion to reveal session titles
    view |> element("button[phx-value-project=\"/home/user/my-project\"]") |> render_click()

    assert render(view) =~ "List the files"
  end
end
