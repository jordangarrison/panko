defmodule Panko.Sessions.SubAgent do
  use Ash.Resource,
    domain: Panko.Sessions,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "sub_agents"
    repo Panko.Repo
  end

  attributes do
    uuid_primary_key :id

    attribute :external_id, :string do
      allow_nil? false
      public? true
    end

    attribute :agent_type, :string do
      allow_nil? false
      public? true
    end

    attribute :description, :string do
      allow_nil? true
      public? true
    end

    attribute :prompt, :string do
      allow_nil? true
      public? true
    end

    attribute :status, Panko.Sessions.SubAgentStatus do
      allow_nil? false
      public? true
    end

    attribute :result, :string do
      allow_nil? true
      public? true
    end

    attribute :spawned_at, :utc_datetime do
      allow_nil? false
      public? true
    end

    attribute :completed_at, :utc_datetime do
      allow_nil? true
      public? true
    end

    create_timestamp :inserted_at
  end

  relationships do
    belongs_to :session, Panko.Sessions.Session do
      allow_nil? false
      public? true
    end
  end

  actions do
    defaults [:read, :destroy]

    create :create do
      primary? true

      accept [
        :external_id,
        :agent_type,
        :description,
        :prompt,
        :status,
        :result,
        :spawned_at,
        :completed_at,
        :session_id
      ]
    end

    update :update do
      primary? true

      accept [
        :external_id,
        :agent_type,
        :description,
        :prompt,
        :status,
        :result,
        :spawned_at,
        :completed_at
      ]
    end
  end
end
