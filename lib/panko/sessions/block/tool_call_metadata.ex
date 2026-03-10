defmodule Panko.Sessions.Block.ToolCallMetadata do
  use Ash.Resource, data_layer: :embedded

  attributes do
    attribute :name, :string, allow_nil?: false, public?: true
    attribute :input, :map, public?: true
    attribute :output, :map, public?: true
  end
end
