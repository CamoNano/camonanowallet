use core_client::rpc::{Rpc, RpcCommands};
use core_client::Account;

pub fn default_representatives() -> Vec<Account> {
    vec![
        // Nano Charts
        "nano_3chartsi6ja8ay1qq9xg3xegqnbg1qx76nouw6jedyb8wx3r4wu94rxap7hg"
            .parse()
            .unwrap(),
        // Kappture
        "nano_3ktybzzy14zxgb6osbhcc155pwk7osbmf5gbh5fo73bsfu9wuiz54t1uozi1"
            .parse()
            .unwrap(),
        // NANO ITALIA
        "nano_1wcxcjbwnnsdpee3d9i365e8bcj1uuyoqg9he5zjpt3r57dnjqe3gdc184ck"
            .parse()
            .unwrap(),
        // Patrick's Self-Hosted Nano Node
        "nano_3patrick68y5btibaujyu7zokw7ctu4onikarddphra6qt688xzrszcg4yuo"
            .parse()
            .unwrap(),
        // RsNano.com - Nano Rust Port
        "nano_1tk8h3yzkibbsti8upkfa69wqafz6mzfzgu8bu5edaay9k7hidqdunpr4tb6"
            .parse()
            .unwrap(),
        // NanoTicker
        "nano_1iuz18n4g4wfp9gf7p1s8qkygxw7wx9qfjq6a9aq68uyrdnningdcjontgar"
            .parse()
            .unwrap(),
        // WeNano
        "nano_1wenanoqm7xbypou7x3nue1isaeddamjdnc3z99tekjbfezdbq8fmb659o7t"
            .parse()
            .unwrap(),
        // Madora
        "nano_3g6ue89jij6bxaz3hodne1c7gzgw77xawpdz4p38siu145u3u17c46or4jeu"
            .parse()
            .unwrap(),
        // gr0vity
        "nano_3msc38fyn67pgio16dj586pdrceahtn75qgnx7fy19wscixrc8dbb3abhbw6"
            .parse()
            .unwrap(),
        // nanowallets.guide
        "nano_1zuksmn4e8tjw1ch8m8fbrwy5459bx8645o9euj699rs13qy6ysjhrewioey"
            .parse()
            .unwrap(),
        // Flying Amigos
        "nano_1xckpezrhg56nuokqh6t1stjca67h37jmrp9qnejjkfgimx1msm9ehuaieuq"
            .parse()
            .unwrap(),
        // NANO TipBot
        "nano_3o7uzba8b9e1wqu5ziwpruteyrs3scyqr761x7ke6w1xctohxfh5du75qgaj"
            .parse()
            .unwrap(),
        // warai
        "nano_33ad5app7jeo6jfe9ure6zsj8yg7knt6c1zrr5yg79ktfzk5ouhmpn6p5d7p"
            .parse()
            .unwrap(),
        // Kedrin
        "nano_15nt4cis8ac184q9mj7bedww9ay9zh5jk5k7sj9ypmz44twjcpz3cn6oijir"
            .parse()
            .unwrap(),
        // ScandiNode - Green, fast & capable!
        "nano_318uu1tsbios3kp4dts5b6zy1y49uyb88jajfjyxwmozht8unaxeb43keork"
            .parse()
            .unwrap(),
        // Yakamoz Node - nano.trade
        "nano_3pg8khw8gs94c1qeq9741n99ubrut8sj3n9kpntim1rm35h4wdzirofazmwt"
            .parse()
            .unwrap(),
        // NanoQuake
        "nano_1kd4h9nqaxengni43xy9775gcag8ptw8ddjifnm77qes1efuoqikoqy5sjq3"
            .parse()
            .unwrap(),
        // My1s Nano Node
        "nano_1my1snode8rwccjxkckjirj65zdxo6g5nhh16fh6sn7hwewxooyyesdsmii3"
            .parse()
            .unwrap(),
        // IronClad - Reliable node built to last
        "nano_11pb5aa6uirs9hoqsg4swnzyehoiqowj94kdpthwkhwufmtd6a11xx35iron"
            .parse()
            .unwrap(),
        // Nano Germany
        "nano_34zuxqdsucurhjrmpc4aixzbgaa4wjzz6bn5ryn56emc9tmd3pnxjoxfzyb6"
            .parse()
            .unwrap(),
    ]
}

fn rpc(url: &str, commands: RpcCommands) -> Rpc {
    Rpc::new(commands, url, None).expect("failed to create RPC: '{url}'")
}

// Thanks u/Xanza for many of these
pub fn default_rpcs() -> Vec<Rpc> {
    vec![
        rpc(
            "https://api.nano.kga.earth/node/proxy",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: true,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: false,
                work_generate: false,
            },
        ),
        rpc(
            "https://app.natrium.io/api",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: false,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: false,
            },
        ),
        rpc(
            "https://node.somenano.com/proxy",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: false,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: false,
            },
        ),
        rpc(
            "https://rainstorm.city/api",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: false,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: true,
            },
        ),
        rpc(
            "https://rpc.wenano.net/api/node-api",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: false,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: false,
            },
        ),
        rpc(
            "https://rpc.nano.to",
            RpcCommands {
                account_balance: true,
                // nano.to nodes don't return raw blocks
                account_history: false,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: true,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: true,
            },
        ),
        rpc(
            "https://solar.nano.to",
            RpcCommands {
                account_balance: true,
                // nano.to nodes don't return raw blocks
                account_history: false,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: true,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                // nano.to nodes have a shared request limit
                work_generate: false,
            },
        ),
        rpc(
            "https://us-1.nano.to",
            RpcCommands {
                account_balance: true,
                // nano.to nodes don't return raw blocks
                account_history: false,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: true,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                // nano.to nodes have a shared request limit
                work_generate: false,
            },
        ),
        rpc(
            "https://us-2.nano.to",
            RpcCommands {
                account_balance: true,
                // nano.to nodes don't return raw blocks
                account_history: false,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: true,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: true,
            },
        ),
        rpc(
            "https://www.bitrequest.app:8020/",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: false,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: false,
            },
        ),
        rpc(
            "http://node.perish.co:9076",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: true,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: false,
            },
        ),
        rpc(
            "https://nanoslo.0x.no/proxy",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: false,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: true,
            },
        ),
        rpc(
            "https://nault.nanos.cc/proxy",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: true,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: true,
            },
        ),
        rpc(
            "https://proxy.nanos.cc/proxy",
            RpcCommands {
                account_balance: true,
                account_history: true,
                account_info: true,
                account_representative: true,
                accounts_balances: true,
                accounts_frontiers: true,
                accounts_receivable: false,
                accounts_representatives: true,
                block_info: true,
                blocks_info: true,
                process: true,
                work_generate: true,
            },
        ),
        // doesn't work
        // rpc(
        //     "http://workers.perish.co",
        //     Commands {
        //         account_balance: false,
        //         account_history: false,
        //         account_info: false,
        //         account_representative: false,
        //         accounts_balances: false,
        //         accounts_frontiers: false,
        //         accounts_receivable: false,
        //         accounts_representatives: false,
        //         block_info: false,
        //         blocks_info: false,
        //         process: false,
        //         work_generate: true,
        //     },
        // ),
        // doesn't work
        // rpc(
        //     "https://worker.nanoriver.cc",
        //     Commands {
        //         account_balance: false,
        //         account_history: false,
        //         account_info: false,
        //         account_representative: false,
        //         accounts_balances: false,
        //         accounts_frontiers: false,
        //         accounts_receivable: false,
        //         accounts_representatives: false,
        //         block_info: false,
        //         blocks_info: false,
        //         process: false,
        //         work_generate: true,
        //     },
        // ),
        // doesn't work
        // rpc(
        //     "https://www.nanolooker.com/api/rpc",
        //     Commands {
        //         account_balance: true,
        //         account_history: true,
        //         account_info: true,
        //         account_representative: true,
        //         accounts_balances: true,
        //         accounts_frontiers: true,
        //         accounts_receivable: true,
        //         accounts_representatives: true,
        //         block_info: true,
        //         blocks_info: true,
        //         process: true,
        //         work_generate: false,
        //     },
        // ),
        // invalid certificate
        // rpc(
        //     "https://solarnanofaucet.space/api",
        //     Commands {
        //         account_balance: true,
        //         account_history: true,
        //         account_info: true,
        //         account_representative: true,
        //         accounts_balances: true,
        //         accounts_frontiers: true,
        //         accounts_receivable: true,
        //         accounts_representatives: true,
        //         block_info: true,
        //         blocks_info: true,
        //         process: true,
        //         work_generate: false,
        //     },
        // ),
    ]
}
