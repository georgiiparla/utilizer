class Node
  attr_accessor :props, :next_node, :prev_node

  def initialize(props, next_node = nil, prev_node = nil)
    @props = props
    @next_node = next_node
    @prev_node = prev_node
  end
end
