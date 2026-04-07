defmodule Panko.Sessions.Block.FileEditMetadata do
  use Ash.Resource, data_layer: :embedded

  attributes do
    attribute :path, :string, allow_nil?: false, public?: true
    attribute :diff, :string, public?: true
  end
end
