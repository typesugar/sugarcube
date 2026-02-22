interface User {
    name: string;
    age: number;
}
function greet(user: User): string {
    return `Hello, ${user.name}`;
}
const users: User[] = [
    {
        name: "Alice",
        age: 30
    },
    {
        name: "Bob",
        age: 25
    }
];
const names = users.map((u)=>u.name);
