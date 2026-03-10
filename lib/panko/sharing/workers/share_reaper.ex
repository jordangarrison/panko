defmodule Panko.Sharing.Workers.ShareReaper do
  @moduledoc """
  Oban worker that periodically reaps expired shares.

  Runs on a cron schedule (hourly by default) and unpublishes any
  shares whose `expires_at` timestamp has passed.
  """
  use Oban.Worker, queue: :shares

  alias Panko.Sharing.Share

  require Ash.Query

  @impl Oban.Worker
  def perform(_job) do
    now = DateTime.utc_now()

    expired_shares =
      Share
      |> Ash.Query.filter(is_shared: true)
      |> Ash.read!()
      |> Enum.filter(fn share ->
        share.expires_at != nil and DateTime.compare(share.expires_at, now) == :lt
      end)

    for share <- expired_shares do
      Panko.Sharing.unpublish_share(share)
    end

    :ok
  end
end
