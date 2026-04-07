defmodule Panko.Application do
  # See https://hexdocs.pm/elixir/Application.html
  # for more information on OTP Applications
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    children =
      [
        PankoWeb.Telemetry,
        Panko.Repo,
        {AshAuthentication.Supervisor, otp_app: :panko},
        {DNSCluster, query: Application.get_env(:panko, :dns_cluster_query) || :ignore},
        {Phoenix.PubSub, name: Panko.PubSub},
        {Oban, Application.fetch_env!(:panko, Oban)},
        maybe_session_watcher(),
        PankoWeb.Endpoint
      ]
      |> Enum.reject(&is_nil/1)

    # See https://hexdocs.pm/elixir/Supervisor.html
    # for other strategies and supported options
    opts = [strategy: :one_for_one, name: Panko.Supervisor]
    Supervisor.start_link(children, opts)
  end

  defp maybe_session_watcher do
    if Application.get_env(:panko, :start_session_watcher, true) do
      {Panko.Sessions.SessionWatcher, []}
    end
  end

  # Tell Phoenix to update the endpoint configuration
  # whenever the application is updated.
  @impl true
  def config_change(changed, _new, removed) do
    PankoWeb.Endpoint.config_change(changed, removed)
    :ok
  end
end
