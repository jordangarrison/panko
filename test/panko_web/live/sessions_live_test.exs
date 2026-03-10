defmodule PankoWeb.SessionsLiveTest do
  use PankoWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  test "renders empty state when no sessions", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/")
    assert render(view) =~ "No sessions found"
  end

  test "renders sessions list", %{conn: conn} do
    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, _session} = Panko.Sessions.import_from_file(path)

    {:ok, _view, html} = live(conn, ~p"/")
    assert html =~ "List the files"
  end
end
