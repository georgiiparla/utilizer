require_relative 'node'

class LinkedList
    attr_accessor :head
    attr_reader :length
    
    def initialize
        @head = nil
        @length = 0
    end
    
    def add(value)
        if @head.nil?
            @head = Node.new(value)
            @head.next_node = @head
            @head.prev_node = @head
        else
            new_node = Node.new(value)
            
            new_node.next_node = @head
            new_node.prev_node = @head.prev_node
            
            @head.prev_node.next_node = new_node
            @head.prev_node = new_node
        end
        
        @length += 1
    end
end