const Fs = require('fs');

const hashLength = 100;

function insert(obj) {

    const fd = Fs.openSync('./db/tools', 'a+');

    const stringify = JSON.stringify(obj);

    const hash = stringify.padEnd(hashLength, ' ');

    const buffer = Buffer.from(hash);

    Fs.writeSync(fd, buffer);

    Fs.closeSync(fd);

}

function find(condition) {

    let bytesRead = 1;
    let index = 0;

    const data = [];

    const fd = Fs.openSync('./db/tools', 'r+');

    while (bytesRead) {

        const position = hashLength * index;

        const buffer = Buffer.from(''.padEnd(hashLength, ' '));

        bytesRead = Fs.readSync(fd, buffer, 0, hashLength, position);

        if (bytesRead) {

            const hash = buffer.toString();

            const stringify = hash.replace(new RegExp(' ', 'g'), '');

            const obj = JSON.parse(stringify);

            if ([obj].some(condition))
                data.push(obj);

        }

        index++;

    }

    Fs.closeSync(fd);

    return data;

}

insert({ name: 'Vitor', age: 21 });
insert({ name: 'Jose', age: 40 });
insert({ name: 'Matheus', age: 21, o: 'teste' });

const data = find(obj => obj.age === 21);

console.log(data)