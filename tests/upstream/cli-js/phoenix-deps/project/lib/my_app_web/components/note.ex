defmodule MyAppWeb.Note do
  @moduledoc """
  A component with colocated CSS.

  This module is not a file of `mix phx.new`. It gives the project one
  file in `_build/dev/phoenix-colocated/my_app/`.
  """
  use Phoenix.Component

  attr :title, :string, required: true
  slot :inner_block, required: true

  def note(assigns) do
    ~H"""
    <style :type={MyAppWeb.ColocatedCSS}>
      .note {
        @apply rounded-box border border-base-300 bg-base-200 p-4 text-sm;
      }
      .note-title {
        @apply font-semibold text-primary;
      }
    </style>
    <div class="note sm:max-w-md">
      <p class="note-title">{@title}</p>
      {render_slot(@inner_block)}
    </div>
    """
  end
end
