defmodule PankoWeb.SessionLiveTest do
  use PankoWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  setup do
    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, session} = Panko.Sessions.import_from_file(path)
    %{session: session}
  end

  test "renders session detail", %{conn: conn, session: session} do
    {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
    assert html =~ "List the files"
  end

  test "shows blocks", %{conn: conn, session: session} do
    {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
    assert html =~ "Bash"
  end

  test "shows back link", %{conn: conn, session: session} do
    {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
    assert html =~ "Back"
  end

  test "shows session metadata", %{conn: conn, session: session} do
    {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
    assert html =~ "messages"
    assert html =~ "blocks"
  end

  test "redirects for invalid session id", %{conn: conn} do
    assert {:error, {:live_redirect, %{to: "/"}}} =
             live(conn, ~p"/sessions/#{Ash.UUID.generate()}")
  end

  describe "with complex session" do
    setup do
      path = Path.join(["test/fixtures", "complex_session.jsonl"])
      {:ok, session} = Panko.Sessions.import_from_file(path)
      %{complex_session: session}
    end

    test "renders thinking blocks", %{conn: conn, complex_session: session} do
      {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
      assert html =~ "Thinking..."
    end

    test "renders file edit blocks", %{conn: conn, complex_session: session} do
      {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
      assert html =~ "Write"
    end

    test "renders sub agent spawn blocks", %{conn: conn, complex_session: session} do
      {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
      assert html =~ "Agent"
    end
  end
end
