defmodule Panko.Sessions.SourceType do
  use Ash.Type.Enum, values: [:claude_code, :codex]
end
