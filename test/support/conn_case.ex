defmodule PankoWeb.ConnCase do
  @moduledoc """
  This module defines the test case to be used by
  tests that require setting up a connection.
  """

  use ExUnit.CaseTemplate

  using do
    quote do
      @endpoint PankoWeb.Endpoint

      use PankoWeb, :verified_routes

      import Plug.Conn
      import Phoenix.ConnTest
      import PankoWeb.ConnCase
    end
  end

  setup tags do
    Panko.DataCase.setup_sandbox(tags)
    {:ok, conn: Phoenix.ConnTest.build_conn()}
  end

  @doc """
  Creates a registered user and returns the user struct.

  The returned user has `__metadata__.token` set, which is required
  for session-based authentication with AshAuthentication.
  """
  def register_user(attrs \\ %{}) do
    params =
      Map.merge(
        %{
          email: "user#{System.unique_integer()}@example.com",
          password: "password123456",
          password_confirmation: "password123456"
        },
        attrs
      )

    {:ok, user} =
      Panko.Accounts.User
      |> Ash.Changeset.for_create(:register_with_password, params)
      |> Ash.create(authorize?: false)

    user
  end

  @doc """
  Logs in a user by putting auth info in the session.
  Returns the updated conn.

  Uses AshAuthentication.Plug.Helpers.store_in_session/2 which is the same
  function used by the AuthController on successful login.
  """
  def log_in_user(conn, user) do
    conn
    |> Phoenix.ConnTest.init_test_session(%{})
    |> AshAuthentication.Plug.Helpers.store_in_session(user)
  end
end
