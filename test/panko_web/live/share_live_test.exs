defmodule PankoWeb.ShareLiveTest do
  use PankoWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  setup do
    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, session} = Panko.Sessions.import_from_file(path)
    {:ok, share} = Panko.Sharing.create_share(session.id)
    %{session: session, share: share}
  end

  test "renders shared session", %{conn: conn, share: share} do
    {:ok, _view, html} = live(conn, ~p"/s/#{share.slug}")
    assert html =~ "List the files"
    assert html =~ "Panko"
  end

  test "shows 404 for invalid slug", %{conn: conn} do
    {:ok, _view, html} = live(conn, ~p"/s/nonexistent")
    assert html =~ "404"
  end

  test "shows session title", %{conn: conn, share: share, session: session} do
    {:ok, _view, html} = live(conn, ~p"/s/#{share.slug}")
    assert html =~ (session.title || "Shared Session")
  end

  test "shows session project", %{conn: conn, share: share, session: session} do
    {:ok, _view, html} = live(conn, ~p"/s/#{share.slug}")
    assert html =~ session.project
  end

  test "shows footer with Panko link", %{conn: conn, share: share} do
    {:ok, _view, html} = live(conn, ~p"/s/#{share.slug}")
    assert html =~ "github.com/jordangarrison/panko"
  end

  test "shows 404 for unpublished share", %{conn: conn, share: share} do
    {:ok, _} = Panko.Sharing.unpublish_share(share)
    {:ok, _view, html} = live(conn, ~p"/s/#{share.slug}")
    assert html =~ "404"
  end

  test "shows expired for expired share", %{conn: conn, session: session} do
    # Create a share that already expired
    {:ok, expired_share} =
      Panko.Sharing.create_share(session.id, %{
        expires_at: DateTime.add(DateTime.utc_now(), -3600, :second)
      })

    {:ok, _view, html} = live(conn, ~p"/s/#{expired_share.slug}")
    assert html =~ "Expired"
  end
end
