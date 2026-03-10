defmodule Panko.Sessions.SubAgentStatus do
  use Ash.Type.Enum, values: [:running, :completed, :failed]
end
