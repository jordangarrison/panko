defmodule PankoWeb.ShareUITest do
  use PankoWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  setup do
    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, session} = Panko.Sessions.import_from_file(path)
    %{session: session}
  end

  describe "SessionLive share button" do
    test "shows Share button when session is not shared", %{conn: conn, session: session} do
      {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
      assert html =~ "Share"
    end

    test "creates share when Share button is clicked", %{conn: conn, session: session} do
      {:ok, view, _html} = live(conn, ~p"/sessions/#{session.id}")

      html =
        view
        |> element("button", "Share")
        |> render_click()

      # After sharing, the "Shared" button should appear
      assert html =~ "Shared"
    end

    test "shows Shared button for already-shared session", %{conn: conn, session: session} do
      {:ok, _share} = Panko.Sharing.create_share(session.id)

      {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
      assert html =~ "Shared"
    end

    test "unpublish changes button back to Reshare", %{conn: conn, session: session} do
      {:ok, _share} = Panko.Sharing.create_share(session.id)

      {:ok, view, _html} = live(conn, ~p"/sessions/#{session.id}")

      # Click "Shared" to open modal, then click "Unpublish"
      html =
        view
        |> element("button", "Unpublish")
        |> render_click()

      assert html =~ "Reshare"
    end

    test "reshare restores the shared state", %{conn: conn, session: session} do
      {:ok, share} = Panko.Sharing.create_share(session.id)
      {:ok, _} = Panko.Sharing.unpublish_share(share)

      {:ok, view, _html} = live(conn, ~p"/sessions/#{session.id}")

      html =
        view
        |> element("button", "Reshare")
        |> render_click()

      assert html =~ "Shared"
    end

    test "share URL contains the slug", %{conn: conn, session: session} do
      {:ok, _share} = Panko.Sharing.create_share(session.id)

      {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
      assert html =~ "/s/"
    end
  end

  describe "SessionsLive share indicator" do
    test "shows Shared badge for shared sessions", %{conn: conn, session: session} do
      {:ok, _share} = Panko.Sharing.create_share(session.id)

      {:ok, view, _html} = live(conn, ~p"/")

      # Expand the project accordion to reveal session badges
      view
      |> element("button[phx-value-project=\"/home/user/my-project\"]")
      |> render_click()

      assert render(view) =~ "Shared"
    end

    test "does not show Shared badge for unshared sessions", %{conn: conn} do
      {:ok, _view, html} = live(conn, ~p"/")
      refute html =~ "badge-success"
    end
  end
end
