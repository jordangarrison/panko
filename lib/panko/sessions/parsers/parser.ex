defmodule Panko.Sessions.Parsers.Parser do
  @moduledoc """
  Behaviour for session file parsers.

  Parsers are pure functions: file path in, session attributes out.
  The returned map must match the shape expected by Session's
  `:upsert_from_import` action.
  """

  @type session_attrs :: %{
          external_id: String.t(),
          source_type: atom(),
          source_path: String.t(),
          project: String.t() | nil,
          title: String.t() | nil,
          started_at: DateTime.t(),
          blocks: [map()],
          sub_agents: [map()]
        }

  @callback source_type() :: atom()
  @callback can_parse?(path :: String.t()) :: boolean()
  @callback parse(path :: String.t()) :: {:ok, session_attrs()} | {:error, term()}
end
