require "test_helper"
require "cfgs_orchestrator"
require "playlist_parser"
require "linked_list"

class TestCfgsOrchestrator < Minitest::Test
  def test_create_pos_cfg_files_from_linked_list_and_loader
    co = CfgsOrchestrator.new(CONFIG[:csgo_cfg_folder])
    ll = LinkedList.new
    p = PlaylistParser.new(CONFIG[:playlists_folder])
    p.apply("mirage.csv", ll)

    co.create_pos_cfg_files_from_linked_list(ll)

    assert_equal 4, Dir.glob(File.join(CONFIG[:csgo_cfg_folder], "utilizer", "p_*.cfg")).size
    co.generate_loader(ll)
    assert File.exist?(File.join(CONFIG[:csgo_cfg_folder], "load.cfg"))
  end

  # def teardown
  #     # Clean up generated cfg files after each test
  #     puts "Check the files, sleeping for 5 seconds..."
  #     sleep(5)
  #     Dir.glob(File.join(CONFIG[:csgo_cfg_folder], "p_*.cfg")).each do |file|
  #         File.delete(file)
  #     end
  # end
end
