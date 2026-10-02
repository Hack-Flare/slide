DEFINE TABLE election_state SCHEMAFULL;
DEFINE FIELD current_term ON election_state TYPE int;
DEFINE FIELD voted_for ON election_state TYPE option<string>;
DEFINE FIELD leader_id ON election_state TYPE option<string>;
