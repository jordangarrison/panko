defmodule Panko.Sessions.Parsers.RegistryTest do
  use ExUnit.Case, async: true

  alias Panko.Sessions.Parsers.Registry

  test "finds ClaudeCode parser for .jsonl files" do
    assert {:ok, Panko.Sessions.Parsers.ClaudeCode} = Registry.find_parser("/tmp/session.jsonl")
  end

  test "returns error for unknown file types" do
    assert {:error, :no_parser_found} = Registry.find_parser("/tmp/session.xml")
  end
end
