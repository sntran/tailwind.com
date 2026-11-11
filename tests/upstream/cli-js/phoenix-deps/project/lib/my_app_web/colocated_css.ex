defmodule MyAppWeb.ColocatedCSS do
  @moduledoc """
  Colocated CSS with no scope: the CSS of a `<style>` tag goes to the
  stylesheet of the application as it is.

  This module is not a file of `mix phx.new`. It is the example of the
  documentation of `Phoenix.LiveView.ColocatedCSS`.
  """
  use Phoenix.LiveView.ColocatedCSS

  @impl true
  def transform("style", _attrs, css, _meta) do
    {:ok, css, []}
  end
end
