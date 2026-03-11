defmodule PankoWeb.Plugs.ApiKeyAuth do
  @moduledoc """
  Optional API key authentication plug.

  If PANKO_API_KEY is set, requests must provide it via:
  - Authorization: Bearer <key> header
  - ?api_key=<key> query parameter
  - Session storage (for browser-based auth)

  If PANKO_API_KEY is not set, all requests pass through.
  """
  import Plug.Conn

  def init(opts), do: opts

  def call(conn, _opts) do
    case Application.get_env(:panko, :api_key) do
      nil -> conn
      "" -> conn
      expected_key -> verify_key(conn, expected_key)
    end
  end

  defp verify_key(conn, expected_key) do
    provided =
      get_bearer_token(conn) ||
        conn.query_params["api_key"] ||
        get_session_key(conn)

    if provided != nil and Plug.Crypto.secure_compare(provided, expected_key) do
      conn
    else
      conn
      |> put_resp_content_type("text/plain")
      |> send_resp(401, "Unauthorized")
      |> halt()
    end
  end

  defp get_bearer_token(conn) do
    case get_req_header(conn, "authorization") do
      ["Bearer " <> token] -> token
      _ -> nil
    end
  end

  defp get_session_key(conn) do
    get_session(conn, :api_key)
  rescue
    # Session may not be initialized (e.g., in non-browser pipelines)
    ArgumentError -> nil
  end
end
