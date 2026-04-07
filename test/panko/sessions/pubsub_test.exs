defmodule Panko.Sessions.PubSubTest do
  use Panko.DataCase, async: false

  test "broadcasts on session import" do
    PankoWeb.Endpoint.subscribe("sessions:imported")

    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, _session} = Panko.Sessions.import_from_file(path)

    assert_receive %Phoenix.Socket.Broadcast{topic: "sessions:imported"}, 1_000
  end
end
