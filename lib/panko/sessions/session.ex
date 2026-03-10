defmodule Panko.Sessions.Session do
  use Ash.Resource,
    domain: Panko.Sessions,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "sessions"
    repo Panko.Repo
  end

  attributes do
    uuid_primary_key :id

    attribute :external_id, :string do
      allow_nil? false
      public? true
    end

    attribute :source_type, Panko.Sessions.SourceType do
      allow_nil? false
      public? true
    end

    attribute :source_path, :string do
      allow_nil? true
      public? true
    end

    attribute :project, :string do
      allow_nil? true
      public? true
    end

    attribute :title, :string do
      allow_nil? true
      public? true
    end

    attribute :started_at, :utc_datetime do
      allow_nil? false
      public? true
    end

    attribute :user_id, :uuid do
      allow_nil? true
      public? true
    end

    attribute :origin_id, :string do
      allow_nil? true
      public? true
    end

    timestamps()
  end

  relationships do
    has_many :blocks, Panko.Sessions.Block do
      sort position: :asc
      public? true
    end

    has_many :sub_agents, Panko.Sessions.SubAgent do
      public? true
    end
  end

  aggregates do
    count :block_count, :blocks

    count :tool_call_count, :blocks do
      filter expr(block_type == :tool_call)
    end

    count :file_edit_count, :blocks do
      filter expr(block_type == :file_edit)
    end

    count :message_count, :blocks do
      filter expr(block_type in [:user_prompt, :assistant_response])
    end

    first :last_activity_at, :blocks, :timestamp do
      sort timestamp: :desc
    end
  end

  identities do
    identity :external_id_source_type, [:external_id, :source_type]
  end

  actions do
    defaults [:read, :destroy]

    create :create do
      primary? true

      accept [
        :external_id,
        :source_type,
        :source_path,
        :project,
        :title,
        :started_at,
        :user_id,
        :origin_id
      ]
    end

    read :list_recent do
      prepare build(sort: [started_at: :desc], limit: 50)
    end
  end
end
