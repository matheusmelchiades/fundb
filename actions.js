const fundb = require('./SGDB')

const types = require('./types')

exports.createDB = ([ name ]) => {

    if (!name)
        return console.log('Invalid name!')

    fundb.createDB(name)

    console.log(`Created Database ${name} with success!`)
}

exports.createTable = ([ table ]) => {

    if (!table)
        return console.log('Input invalid!')

    fundb.createTable(table)

}

exports.listDatabases = () => {

    const dbs = fundb.listDatabases()

    if (!dbs.length)
        return console.log('No databases!\n')

    console.log('All databases\n')

    dbs.map(db => console.log(` 🛢\  >> ${db}\n`))
    
}

exports.findByIndex = ([ table, index ]) => {

    const users = fundb.findByIndex(table, index)

    console.log(' Result: \n')
    console.log(users)
    console.log()
}

exports.insert = ([ table, name, description ]) => {

    const obj = { name, description }

    fundb.insert(table, obj)

}

exports.setDatabase = ([ dbName ], rl) => {

    console.log(fundb)

    const db = fundb.setCurrentDatabase(dbName)

    if (!db)
        console.log('Database not exists')

    rl.setPrompt(`🛢\  >> ${db} 👉 `)

}

exports.listTables = () => {

    const dbName = fundb.getCurrentDatabase()

    if (!dbName)
        return console.log('Select a database before!')

    const tables = fundb.listTables()

    if (!tables.length)
        return console.log('\nNo tables!\n')

    console.log(`All table from ${dbName}\n`)

    tables.map(table => console.log(`  📂\ >> ${table}\n`))

}

exports.help = () => {

    console.log('\n')

    for (const action in types) {

        console.log(types[action].help, '\n')

    }

}