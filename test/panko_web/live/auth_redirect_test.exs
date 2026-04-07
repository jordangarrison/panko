defmodule PankoWeb.AuthRedirectTest do
  use PankoWeb.ConnCase, async: true
  import Phoenix.LiveViewTest

  describe "unauthenticated access" do
    test "redirects / to sign-in", %{conn: conn} do
      assert {:error, {:redirect, %{to: "/sign-in"}}} = live(conn, ~p"/")
    end

    test "redirects /sessions/:id to sign-in", %{conn: conn} do
      assert {:error, {:redirect, %{to: "/sign-in"}}} =
               live(conn, ~p"/sessions/00000000-0000-0000-0000-000000000000")
    end
  end

  describe "public share access" do
    test "/s/:slug does not redirect to sign-in", %{conn: conn} do
      result = live(conn, ~p"/s/nonexistent")

      case result do
        {:error, {:redirect, %{to: "/sign-in"}}} ->
          flunk("Share route should not redirect to sign-in")

        _ ->
          assert true
      end
    end
  end
end
