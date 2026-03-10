defmodule Panko.Sessions.Parsers.Registry do
  @moduledoc """
  Finds the appropriate parser for a given file path.
  """

  @parsers [
    Panko.Sessions.Parsers.ClaudeCode
  ]

  @spec find_parser(String.t()) :: {:ok, module()} | {:error, :no_parser_found}
  def find_parser(path) do
    case Enum.find(@parsers, & &1.can_parse?(path)) do
      nil -> {:error, :no_parser_found}
      parser -> {:ok, parser}
    end
  end

  @spec parsers() :: [module()]
  def parsers, do: @parsers
end
