defmodule PankoWeb.Plugs.ApiKeyAuthTest do
  use PankoWeb.ConnCase, async: false

  alias PankoWeb.Plugs.ApiKeyAuth

  setup do
    original = Application.get_env(:panko, :api_key)

    on_exit(fn ->
      Application.put_env(:panko, :api_key, original)
    end)

    :ok
  end

  describe "when no API key is configured" do
    test "passes through when api_key is nil", %{conn: conn} do
      Application.put_env(:panko, :api_key, nil)
      conn = ApiKeyAuth.call(conn, [])
      refute conn.halted
    end

    test "passes through when api_key is empty string", %{conn: conn} do
      Application.put_env(:panko, :api_key, "")
      conn = ApiKeyAuth.call(conn, [])
      refute conn.halted
    end
  end

  describe "when API key is configured" do
    setup do
      Application.put_env(:panko, :api_key, "secret123")
      :ok
    end

    test "blocks when no key is provided", %{conn: conn} do
      conn =
        conn
        |> fetch_query_params()
        |> ApiKeyAuth.call([])

      assert conn.halted
      assert conn.status == 401
    end

    test "passes with correct bearer token", %{conn: conn} do
      conn =
        conn
        |> fetch_query_params()
        |> put_req_header("authorization", "Bearer secret123")
        |> ApiKeyAuth.call([])

      refute conn.halted
    end

    test "blocks with incorrect bearer token", %{conn: conn} do
      conn =
        conn
        |> fetch_query_params()
        |> put_req_header("authorization", "Bearer wrong_key")
        |> ApiKeyAuth.call([])

      assert conn.halted
      assert conn.status == 401
    end

    test "passes with correct query parameter", %{conn: conn} do
      conn =
        conn
        |> Map.put(:query_params, %{"api_key" => "secret123"})
        |> ApiKeyAuth.call([])

      refute conn.halted
    end

    test "blocks with incorrect query parameter", %{conn: conn} do
      conn =
        conn
        |> Map.put(:query_params, %{"api_key" => "wrong_key"})
        |> ApiKeyAuth.call([])

      assert conn.halted
      assert conn.status == 401
    end

    test "prefers bearer token over query parameter", %{conn: conn} do
      conn =
        conn
        |> Map.put(:query_params, %{"api_key" => "wrong_key"})
        |> put_req_header("authorization", "Bearer secret123")
        |> ApiKeyAuth.call([])

      refute conn.halted
    end
  end
end
