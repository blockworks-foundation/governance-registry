use std::sync::Arc;

use solana_program::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use spl_governance::instruction::AddSignatoryAuthority;
use spl_governance::state::enums::{MintMaxVoterWeightSource, VoteThreshold, VoteTipping};
use spl_governance::state::realm::GoverningTokenConfigAccountArgs;
use spl_governance::state::realm_config::GoverningTokenType;
use spl_governance::state::{proposal, vote_record};

use crate::*;

#[derive(Clone)]
pub struct GovernanceCookie {
    pub solana: Arc<solana::SolanaCookie>,
    pub program_id: Pubkey,
}

#[derive(Clone)]
pub struct GovernanceRealmCookie {
    pub governance: GovernanceCookie,
    pub authority: Pubkey,
    pub realm: Pubkey,
    pub realm_config: Pubkey,
    pub community_token_mint: MintCookie,
    pub community_token_account: Pubkey,
}

#[derive(Clone, Debug)]
pub struct TokenOwnerRecordCookie {
    pub address: Pubkey,
}

pub struct AccountGovernanceCookie {
    pub address: Pubkey,
    pub governed_account: Pubkey,
}

pub struct MintGovernanceCookie {
    pub address: Pubkey,
    pub governed_mint: Pubkey,
}

#[derive(Debug)]
pub struct ProposalCookie {
    pub address: Pubkey,
    pub owner_token_owner_record: Pubkey,
}

impl GovernanceCookie {
    pub async fn create_realm(
        &self,
        name: &str,
        realm_authority: Pubkey,
        community_token_mint: &MintCookie,
        payer: &Keypair,
        voter_weight_addin: &Pubkey,
    ) -> GovernanceRealmCookie {
        let realm = Pubkey::find_program_address(
            &[b"governance".as_ref(), name.as_ref()],
            &self.program_id,
        )
        .0;
        let community_token_account = Pubkey::find_program_address(
            &[
                b"governance".as_ref(),
                &realm.to_bytes(),
                &community_token_mint.pubkey.unwrap().to_bytes(),
            ],
            &self.program_id,
        )
        .0;
        let realm_config = Pubkey::find_program_address(
            &[b"realm-config".as_ref(), &realm.to_bytes()],
            &self.program_id,
        )
        .0;

        let community_token_config_args = GoverningTokenConfigAccountArgs {
            voter_weight_addin: Some(*voter_weight_addin),
            max_voter_weight_addin: None,
            token_type: GoverningTokenType::default(),
        };

        let community_mint_max_voter_weight_source = MintMaxVoterWeightSource::FULL_SUPPLY_FRACTION;
        let instructions = vec![spl_governance::instruction::create_realm(
            &self.program_id,
            &realm_authority,
            &community_token_mint.pubkey.unwrap(),
            &payer.pubkey(),
            None,
            Some(community_token_config_args),
            None,
            name.to_string(),
            0,
            community_mint_max_voter_weight_source,
        )];

        let signer = Keypair::from_base58_string(&payer.to_base58_string());

        self.solana
            .process_transaction(&instructions, Some(&[&signer]))
            .await
            .unwrap();

        GovernanceRealmCookie {
            governance: self.clone(),
            authority: realm_authority,
            realm,
            realm_config,
            community_token_mint: community_token_mint.clone(),
            community_token_account,
        }
    }
}

impl GovernanceRealmCookie {
    pub async fn create_token_owner_record(
        &self,
        owner: Pubkey,
        payer: &Keypair,
    ) -> TokenOwnerRecordCookie {
        let record = Pubkey::find_program_address(
            &[
                b"governance".as_ref(),
                &self.realm.to_bytes(),
                &self.community_token_mint.pubkey.unwrap().to_bytes(),
                &owner.to_bytes(),
            ],
            &self.governance.program_id,
        )
        .0;

        let instructions = vec![spl_governance::instruction::create_token_owner_record(
            &self.governance.program_id,
            &self.realm,
            &owner,
            &self.community_token_mint.pubkey.unwrap(),
            &payer.pubkey(),
        )];

        let signer = Keypair::from_base58_string(&payer.to_base58_string());

        self.governance
            .solana
            .process_transaction(&instructions, Some(&[&signer]))
            .await
            .unwrap();

        TokenOwnerRecordCookie { address: record }
    }

    #[allow(dead_code)]
    pub async fn create_account_governance(
        &self,
        governed_account: Pubkey,
        voter: &VoterCookie,
        authority: &Keypair,
        payer: &Keypair,
        vwr_instruction: Instruction,
    ) -> AccountGovernanceCookie {
        let account_governance = spl_governance::state::governance::get_governance_address(
            &self.governance.program_id,
            &self.realm,
            &governed_account,
        );

        let instructions = vec![
            vwr_instruction,
            spl_governance::instruction::create_governance(
                &self.governance.program_id,
                &self.realm,
                Some(&governed_account),
                &voter.token_owner_record,
                &payer.pubkey(),
                &authority.pubkey(),
                Some(voter.voter_weight_record),
                spl_governance::state::governance::GovernanceConfig {
                    min_community_weight_to_create_proposal: 1000,
                    min_transaction_hold_up_time: 0,
                    min_council_weight_to_create_proposal: 1,
                    community_vote_threshold: VoteThreshold::YesVotePercentage(50),
                    voting_base_time: 600,
                    community_vote_tipping: VoteTipping::Strict,
                    council_vote_threshold: VoteThreshold::YesVotePercentage(50),
                    council_veto_vote_threshold: VoteThreshold::Disabled,
                    council_vote_tipping: VoteTipping::Disabled,
                    community_veto_vote_threshold: VoteThreshold::Disabled,
                    voting_cool_off_time: 0,
                    deposit_exempt_proposal_count: 10,
                },
            ),
        ];

        let signer1 = Keypair::from_base58_string(&payer.to_base58_string());
        let signer2 = Keypair::from_base58_string(&authority.to_base58_string());

        self.governance
            .solana
            .process_transaction(&instructions, Some(&[&signer1, &signer2]))
            .await
            .unwrap();

        AccountGovernanceCookie {
            address: account_governance,
            governed_account,
        }
    }

    #[allow(dead_code)]
    pub async fn create_mint_governance(
        &self,
        governed_mint: Pubkey,
        governed_mint_authority: &Keypair,
        voter: &VoterCookie,
        authority: &Keypair,
        payer: &Keypair,
        vwr_instruction: Instruction,
    ) -> MintGovernanceCookie {
        let mint_governance = spl_governance::state::governance::get_mint_governance_address(
            &self.governance.program_id,
            &self.realm,
            &governed_mint,
        );

        let instructions = vec![
            vwr_instruction,
            spl_governance::instruction::create_mint_governance(
                &self.governance.program_id,
                &self.realm,
                &governed_mint,
                &governed_mint_authority.pubkey(),
                &voter.token_owner_record,
                &payer.pubkey(),
                &authority.pubkey(),
                Some(voter.voter_weight_record),
                spl_governance::state::governance::GovernanceConfig {
                    min_community_weight_to_create_proposal: 1000,
                    min_transaction_hold_up_time: 0,
                    min_council_weight_to_create_proposal: 1,
                    community_vote_threshold: VoteThreshold::YesVotePercentage(50),
                    voting_base_time: 600,
                    community_vote_tipping: VoteTipping::Strict,
                    council_vote_threshold: VoteThreshold::YesVotePercentage(50),
                    council_veto_vote_threshold: VoteThreshold::Disabled,
                    council_vote_tipping: VoteTipping::Disabled,
                    community_veto_vote_threshold: VoteThreshold::Disabled,
                    voting_cool_off_time: 0,
                    deposit_exempt_proposal_count: 10,
                },
                true,
            ),
        ];

        let signer1 = Keypair::from_base58_string(&payer.to_base58_string());
        let signer2 = Keypair::from_base58_string(&authority.to_base58_string());
        let signer3 = Keypair::from_base58_string(&governed_mint_authority.to_base58_string());

        self.governance
            .solana
            .process_transaction(&instructions, Some(&[&signer1, &signer2, &signer3]))
            .await
            .unwrap();

        MintGovernanceCookie {
            address: mint_governance,
            governed_mint,
        }
    }

    #[allow(dead_code)]
    pub async fn create_proposal(
        &self,
        governance: Pubkey,
        authority: &Keypair,
        voter: &VoterCookie,
        payer: &Keypair,
        vwr_instruction: Instruction,
    ) -> std::result::Result<ProposalCookie, BanksClientError> {
        let proposal_seed = Pubkey::new_unique();
        let proposal = spl_governance::state::proposal::get_proposal_address(
            &self.governance.program_id,
            &governance,
            &self.community_token_mint.pubkey.unwrap(),
            &proposal_seed,
        );
        let add_signatory_authority = AddSignatoryAuthority::ProposalOwner {
            governance_authority: authority.pubkey(),
            token_owner_record: voter.token_owner_record,
        };
        let instructions = vec![
            vwr_instruction,
            spl_governance::instruction::create_proposal(
                &self.governance.program_id,
                &governance,
                &voter.token_owner_record,
                &authority.pubkey(),
                &payer.pubkey(),
                Some(voter.voter_weight_record),
                &self.realm,
                "test proposal".into(),
                "description".into(),
                &self.community_token_mint.pubkey.unwrap(),
                proposal::VoteType::SingleChoice,
                vec!["yes".into()],
                true,
                &proposal_seed,
            ),
            spl_governance::instruction::add_signatory(
                &self.governance.program_id,
                &proposal,
                &voter.token_owner_record,
                &add_signatory_authority,
                &payer.pubkey(),
                &authority.pubkey(),
            ),
            spl_governance::instruction::sign_off_proposal(
                &self.governance.program_id,
                &self.realm,
                &governance,
                &proposal,
                &authority.pubkey(),
                None,
            ),
        ];

        let signer1 = Keypair::from_base58_string(&payer.to_base58_string());
        let signer2 = Keypair::from_base58_string(&authority.to_base58_string());

        self.governance
            .solana
            .process_transaction(&instructions, Some(&[&signer1, &signer2]))
            .await?;

        Ok(ProposalCookie {
            address: proposal,
            owner_token_owner_record: voter.token_owner_record,
        })
    }

    #[allow(dead_code)]
    pub async fn cast_vote(
        &self,
        governance: Pubkey,
        proposal: &ProposalCookie,
        voter: &VoterCookie,
        authority: &Keypair,
        payer: &Keypair,
        vwr_instruction: Instruction,
    ) -> std::result::Result<(), BanksClientError> {
        let instructions = vec![
            vwr_instruction,
            spl_governance::instruction::cast_vote(
                &self.governance.program_id,
                &self.realm,
                &governance,
                &proposal.address,
                &proposal.owner_token_owner_record,
                &voter.token_owner_record,
                &authority.pubkey(),
                &self.community_token_mint.pubkey.unwrap(),
                &payer.pubkey(),
                Some(voter.voter_weight_record),
                None,
                vote_record::Vote::Approve(vec![vote_record::VoteChoice {
                    rank: 0,
                    weight_percentage: 100,
                }]),
            ),
        ];

        let signer1 = Keypair::from_base58_string(&payer.to_base58_string());
        let signer2 = Keypair::from_base58_string(&authority.to_base58_string());

        self.governance
            .solana
            .process_transaction(&instructions, Some(&[&signer1, &signer2]))
            .await
    }

    #[allow(dead_code)]
    pub async fn relinquish_vote(
        &self,
        governance: Pubkey,
        proposal: &ProposalCookie,
        token_owner_record: Pubkey,
        authority: &Keypair,
        beneficiary: Pubkey,
        goverenance_realm: GovernanceRealmCookie,
    ) -> std::result::Result<(), BanksClientError> {
        let instructions = vec![spl_governance::instruction::relinquish_vote(
            &self.governance.program_id,
            &goverenance_realm.realm,
            &governance,
            &proposal.address,
            &token_owner_record,
            &self.community_token_mint.pubkey.unwrap(),
            Some(authority.pubkey()),
            Some(beneficiary),
        )];

        let signer = Keypair::from_base58_string(&authority.to_base58_string());

        self.governance
            .solana
            .process_transaction(&instructions, Some(&[&signer]))
            .await
    }
}
