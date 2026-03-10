defmodule Panko.Sessions.Block.SubAgentSpawnMetadata do
  use Ash.Resource, data_layer: :embedded

  attributes do
    attribute :agent_id, :string, public?: true
    attribute :agent_type, :string, public?: true
    attribute :description, :string, public?: true
  end
end
